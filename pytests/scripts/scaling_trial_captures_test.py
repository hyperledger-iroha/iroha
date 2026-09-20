"""Actual flat resource captures and bounded custody; no native process execution."""
import os
from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'scripts/nexus'))
from resource_replay_test import Fixture
import scaling_trial_captures as captures


@pytest.fixture
def case(tmp_path):
    f=Fixture(tmp_path.resolve());checks=[]
    owner=captures.TrialCaptures(f.directory,f.allocation,lambda:checks.append(True))
    yield f,owner,checks
    owner.close()


def test_exact_original_flat_census_brackets_real_resource_replay(case):
    f,owner,checks=case
    before=owner.census
    result=f.run()
    assert owner.verify()==before
    assert before.files==result.capture_file_count==207
    assert before.bytes==result.capture_bytes
    assert len(owner._chain)<=65 and len(checks)>before.files
    assert not {'rss_bytes','throughput','queue_size'} & set(before.__dataclass_fields__)
    owner.close()
    assert f.directory.is_dir() and len(list(f.directory.iterdir()))==before.files


@pytest.mark.parametrize('kind',['replace','rewrite','remove','extra','symlink','hardlink','mode','directory','ancestor'])
def test_any_original_file_or_namespace_change_is_terminal(case,kind):
    f,owner,_=case;first=sorted(f.directory.iterdir())[0]
    if kind=='replace':
        raw=first.read_bytes();first.rename(first.with_name('detached'));first.write_bytes(raw);first.chmod(0o600)
    elif kind=='rewrite':first.write_bytes(first.read_bytes())
    elif kind=='remove':first.unlink()
    elif kind=='extra':(f.directory/'extra.json').write_bytes(b'{}')
    elif kind=='symlink':
        first.rename(first.with_name('detached'));first.symlink_to(first.with_name('detached'))
    elif kind=='hardlink':os.link(first,first.with_name('second'))
    elif kind=='mode':first.chmod(0o644)
    elif kind=='directory':
        f.directory.rename(f.directory.with_name('detached'));f.directory.mkdir(mode=0o700)
    else:
        parent=f.directory.parent;parent.rename(parent.with_name(parent.name+'-detached'));parent.mkdir(mode=0o700)
    with pytest.raises((captures.TrialCaptureError,OSError,ValueError)):owner.verify()
    with pytest.raises(captures.TrialCaptureError):owner.verify()


@pytest.mark.parametrize('kind',['rewrite','replace'])
def test_earlier_member_mutation_during_later_body_read_fails(case,monkeypatch,kind):
    f,owner,_=case;names=sorted(path.name for path in f.directory.iterdir());first=f.directory/names[0]
    original=captures._read_file;mutated=[]
    def read(parent,name,*args):
        value=original(parent,name,*args)
        if name==names[-1] and not mutated:
            mutated.append(True)
            raw=first.read_bytes()
            if kind=='replace':first.rename(first.with_name('detached'))
            first.write_bytes(raw);first.chmod(0o600)
        return value
    monkeypatch.setattr(captures,'_read_file',read)
    with pytest.raises(captures.TrialCaptureError):owner.verify()
    assert mutated and first.read_bytes()


def test_final_runtime_callback_cannot_mutate_an_earlier_member(case):
    f,owner,_=case;first=sorted(f.directory.iterdir())[0];calls=[]
    def guard():
        calls.append(True)
        # First pre-scan plus two per member, then the final pre-metadata guard.
        if len(calls)==2+2*owner.census.files:first.write_bytes(first.read_bytes())
    owner._guard=guard
    with pytest.raises(captures.TrialCaptureError):owner.verify()
    assert len(calls)==2+2*207


def test_guard_reentry_cannot_recover_or_publish_census(case):
    _,owner,_=case;attempts=[]
    def guard():
        if not attempts:
            attempts.append(True)
            with pytest.raises(captures.TrialCaptureError):owner.verify()
    owner._guard=guard
    with pytest.raises(captures.TrialCaptureError):owner.verify()
    assert attempts


@pytest.mark.parametrize('kind',['badname','missing','excess','wrongsequence','wrongpeer','oversize'])
def test_initial_scope_is_exact_and_bounded(tmp_path,kind):
    f=Fixture(tmp_path.resolve());first=sorted(f.directory.iterdir())[0]
    if kind=='badname':first.rename(first.with_name('unknown'))
    elif kind=='missing':first.unlink()
    elif kind=='excess':(f.directory/'extra').write_bytes(b'x')
    elif kind=='wrongsequence':first.rename(first.with_name('sample-0000999999-peer-0000-status.body'))
    elif kind=='wrongpeer':first.rename(first.with_name('preflight-0000000000-peer-0004-status.body'))
    else:
        status=next(path for path in f.directory.iterdir() if path.name.endswith('status.body'))
        status.write_bytes(b'x'*(f.allocation.policy.status_body_bytes+1))
    before=set(os.listdir('/dev/fd'))
    with pytest.raises((captures.TrialCaptureError,ValueError)):
        captures.TrialCaptures(f.directory,f.allocation,lambda:None)
    assert set(os.listdir('/dev/fd'))==before


def test_original_transfer_survives_source_close_and_uses_public_guard(case):
    f,owner,checks=case;public=[];expected=owner.census
    destination=owner.transfer(lambda:public.append(True))
    assert type(destination) is captures.TransferredCaptures
    assert not set(row[0] for row in owner._chain)&set(row[0] for row in destination._chain)
    owner.close();count=len(checks)
    assert destination.verify()==expected and len(checks)==count and public
    destination.close()
    assert f.directory.is_dir()


def test_transfer_has_no_public_path_or_census_constructor(case):
    f,owner,_=case
    with pytest.raises(captures.TrialCaptureError):captures.TransferredCaptures(f.directory,owner.census)
    destination=owner.transfer(lambda:None)
    try:
        with pytest.raises(captures.TrialCaptureError):owner.transfer(lambda:None)
        with pytest.raises(captures.TrialCaptureError):destination.transfer(lambda:None)
    finally:destination.close()


def test_original_deadline_remains_required_through_public_transfer_callback(case):
    _,owner,_=case;expired=[]
    def source_guard():
        if expired:raise captures.TrialCaptureError('expired source deadline')
    owner._guard=source_guard;before=set(os.listdir('/dev/fd'))
    with pytest.raises(captures.TrialCaptureError):owner.transfer(lambda:expired.append(True))
    assert set(os.listdir('/dev/fd'))==before
    with pytest.raises(captures.TrialCaptureError):owner.verify()


def test_failed_partial_duplication_preserves_originals_and_leaks_no_handles(case,monkeypatch):
    f,owner,_=case;original=os.dup;calls=[];before=set(os.listdir('/dev/fd'))
    def duplicate(fd):
        calls.append(fd)
        if len(calls)==3:raise OSError('fixture duplication failure')
        return original(fd)
    monkeypatch.setattr(os,'dup',duplicate)
    with pytest.raises(OSError):owner.transfer(lambda:None)
    assert len(calls)==3 and set(os.listdir('/dev/fd'))==before and f.directory.is_dir()


@pytest.mark.parametrize('kind',['rewrite','replace','ancestor'])
def test_transferred_original_custody_rejects_later_mutation(case,kind):
    f,owner,_=case;destination=owner.transfer(lambda:None);owner.close()
    first=sorted(f.directory.iterdir())[0]
    if kind=='rewrite':first.write_bytes(first.read_bytes())
    elif kind=='replace':
        raw=first.read_bytes();first.rename(first.with_name('detached'));first.write_bytes(raw);first.chmod(0o600)
    else:
        parent=f.directory.parent;parent.rename(parent.with_name(parent.name+'-detached'));parent.mkdir(mode=0o700)
    try:
        with pytest.raises((captures.TrialCaptureError,OSError,ValueError)):destination.verify()
    finally:destination.close()


def test_transfer_close_never_closes_a_reused_foreign_descriptor(case):
    f,owner,_=case;destination=owner.transfer(lambda:None)
    fd=destination._chain[0][0];original_root=owner._chain[0][0]
    foreign=os.open(sorted(f.directory.iterdir())[0],os.O_RDONLY)
    try:
        os.dup2(foreign,fd)
        with pytest.raises(captures.TrialCaptureError):destination.close()
        assert os.fstat(fd).st_ino==os.fstat(foreign).st_ino
        assert len(destination._chain)==1
        os.dup2(original_root,fd)
        destination.close()
    finally:os.close(foreign)


def test_duplicate_admission_closes_created_foreign_duplicate_not_borrowed_source(case,monkeypatch):
    f,owner,_=case;source=owner._chain[0][0];original_dup=os.dup
    retained_original=original_dup(source);foreign=os.open(sorted(f.directory.iterdir())[0],os.O_RDONLY)
    before=set(os.listdir('/dev/fd'));created=[]
    def duplicate(fd):
        if not created:os.dup2(foreign,source)
        result=original_dup(fd);created.append(result);return result
    monkeypatch.setattr(os,'dup',duplicate)
    try:
        with pytest.raises(captures.TrialCaptureError):owner.transfer(lambda:None)
        assert len(created)==1 and set(os.listdir('/dev/fd'))==before
        assert os.fstat(source).st_ino==os.fstat(foreign).st_ino
        with pytest.raises(OSError):os.fstat(created[0])
        assert f.directory.is_dir()
    finally:
        os.dup2(retained_original,source);os.close(retained_original);os.close(foreign)


def test_transferred_public_path_and_allocation_accessors_retain_originals(case):
    f,owner,_=case;destination=owner.transfer(lambda:None);owner.close()
    try:
        assert destination.directory==f.directory and destination.allocation is f.allocation
        f.directory.rename(f.directory.with_name('detached'))
        with pytest.raises(captures.TrialCaptureError):_=destination.directory
        with pytest.raises(captures.TrialCaptureError):_=destination.allocation
    finally:destination.close()
