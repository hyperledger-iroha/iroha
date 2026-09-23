"""Actual original-descriptor/ambiguity controls; no SDK/native execution."""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import sys
import pytest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_input_files as owner
from sorafs_javascript_archive import ArchiveError


def test_streamed_original_retains_no_native_size_buffer_and_uses_bounded_reads(tmp_path,monkeypatch):
    path=tmp_path/'inert.node';raw=b'a'*(owner.CHUNK*2+3);path.write_bytes(raw)
    real=os.pread;observed=[]
    def read(fd,count,offset):observed.append(count);return real(fd,count,offset)
    monkeypatch.setattr(os,'pread',read)
    with_file=owner.HeldInputFile(path,len(raw),retain_bytes=False)
    try:
        assert with_file.identity==(hashlib.sha256(raw).hexdigest(),len(raw))
        with pytest.raises(ArchiveError):with_file.raw
        with_file.recheck();assert max(observed)==owner.CHUNK
    finally:with_file.close()


@pytest.mark.parametrize('mutation',('bytes','restored_bytes_mtime','replacement','hardlink','symlink','mode','grow','ancestor'))
def test_original_file_and_ancestry_changes_refuse_permanently(tmp_path,mutation):
    parent=tmp_path/'original';parent.mkdir();path=parent/'input';path.write_bytes(b'original')
    instance=owner.HeldInputFile(path,16);before=path.stat()
    try:
        if mutation=='bytes':path.write_bytes(b'changed!')
        elif mutation=='restored_bytes_mtime':
            path.write_bytes(b'changed!');path.write_bytes(b'original');os.utime(path,ns=(before.st_atime_ns,before.st_mtime_ns))
        elif mutation=='replacement':path.unlink();path.write_bytes(b'original')
        elif mutation=='hardlink':os.link(path,parent/'alias')
        elif mutation=='symlink':path.rename(parent/'saved');path.symlink_to(parent/'saved')
        elif mutation=='mode':path.chmod(0o600)
        elif mutation=='grow':path.write_bytes(b'original larger')
        elif mutation=='ancestor':parent.rename(tmp_path/'saved');parent.mkdir();path.write_bytes(b'original')
        with pytest.raises(ArchiveError):instance.recheck()
        with pytest.raises(ArchiveError):instance.recheck()
        with pytest.raises(ArchiveError):instance.descriptor
    finally:instance.close()


def test_external_sibling_activity_is_not_ancestor_replacement(tmp_path):
    path=tmp_path/'input';path.write_bytes(b'original');instance=owner.HeldInputFile(path,8)
    try:
        (tmp_path/'sibling').write_bytes(b'work');instance.recheck()
    finally:instance.close()


@pytest.mark.parametrize('maximum',(0,-1,True,1024**3+1))
def test_input_bound_is_exact_integer_before_file_acquisition(tmp_path,maximum,monkeypatch):
    def forbidden(*a,**k):raise AssertionError('invalid capacity opened a path')
    monkeypatch.setattr(os,'open',forbidden)
    with pytest.raises(ArchiveError):owner.HeldInputFile(tmp_path/'input',maximum)


def test_size_over_declared_bound_closes_all_original_handles(tmp_path,monkeypatch):
    path=tmp_path/'input';path.write_bytes(b'original');real=os.open;opened=[]
    def opening(*a,**k):fd=real(*a,**k);opened.append(fd);return fd
    monkeypatch.setattr(os,'open',opening)
    with pytest.raises(ArchiveError):owner.HeldInputFile(path,7)
    assert opened
    for fd in opened:
        with pytest.raises(OSError):os.fstat(fd)


@pytest.mark.parametrize('at',('leaf','parent'))
def test_ambiguous_close_detaches_all_and_preserves_actual_reused_descriptor(tmp_path,monkeypatch,at):
    path=tmp_path/'input';path.write_bytes(b'original');instance=owner.HeldInputFile(path,8)
    retained=(*instance._parent[2],instance.descriptor);target=retained[-1 if at=='leaf' else -2]
    replacement=tmp_path/'replacement';replacement.write_bytes(b'foreign sentinel')
    real=os.close;attempted=[];reused=[];cause=OSError('closed before error')
    def closing(fd):
        attempted.append(fd);real(fd)
        if fd==target:
            fresh=os.open(replacement,os.O_RDONLY);assert fresh==target;reused.append(fresh);raise cause
    monkeypatch.setattr(os,'close',closing)
    with pytest.raises(ArchiveError) as raised:instance.close()
    assert raised.value.cleanup_errors==(cause,)
    assert all(attempted.count(fd)==1 for fd in retained)
    instance.close();assert all(attempted.count(fd)==1 for fd in retained)
    assert os.fstat(reused[0]).st_size==len(b'foreign sentinel')
    assert os.pread(reused[0],16,0)==b'foreign sentinel'
    real(reused[0])


def test_multiple_close_failures_still_attempt_every_original_once(tmp_path,monkeypatch):
    path=tmp_path/'input';path.write_bytes(b'original');instance=owner.HeldInputFile(path,8)
    held=(*instance._parent[2],instance.descriptor);real=os.close;seen=[];errors=[]
    def closing(fd):
        seen.append(fd);real(fd)
        if fd in held[-2:]:
            error=OSError(f'ambiguous {fd}');errors.append(error);raise error
    monkeypatch.setattr(os,'close',closing)
    with pytest.raises(ArchiveError) as caught:instance.close()
    assert seen==list(reversed(held)) and caught.value.cleanup_errors==tuple(errors)
    instance.close();assert len(seen)==len(held)


@pytest.mark.parametrize('mode',('close','swallowed_reentry','swallowed_property'))
def test_close_or_swallowed_reentry_cannot_turn_outer_observation_into_success(tmp_path,monkeypatch,mode):
    path=tmp_path/'input';path.write_bytes(b'original');instance=owner.HeldInputFile(path,8)
    fd=instance.descriptor;real=os.fstat;armed=True
    def observing(number):
        nonlocal armed
        value=real(number)
        if number==fd and armed:
            armed=False
            if mode=='close':instance.close()
            elif mode=='swallowed_property':
                with pytest.raises(ArchiveError):instance.descriptor
            else:
                with pytest.raises(ArchiveError):instance.recheck()
        return value
    monkeypatch.setattr(os,'fstat',observing)
    try:
        with pytest.raises((ArchiveError,TypeError,OSError)):instance.recheck()
        with pytest.raises(ArchiveError):instance.recheck()
        with pytest.raises(ArchiveError):instance.descriptor
    finally:instance.close()


def test_acquisition_fstat_failure_preserves_original_cause_and_drains_ancestry(tmp_path,monkeypatch):
    path=tmp_path/'input';path.write_bytes(b'original');real_open=os.open;real_stat=os.fstat;opened=[];leaf=[]
    original=KeyboardInterrupt('original acquisition interrupted');cause=RuntimeError('original explicit cause');original.__cause__=cause
    def opening(name,*a,**k):
        fd=real_open(name,*a,**k);opened.append(fd)
        if name=='input':leaf.append(fd)
        return fd
    def observing(fd):
        if fd in leaf:raise original
        return real_stat(fd)
    monkeypatch.setattr(os,'open',opening);monkeypatch.setattr(os,'fstat',observing)
    with pytest.raises(KeyboardInterrupt) as caught:owner.HeldInputFile(path,8)
    assert caught.value is original and caught.value.__cause__ is cause
    for fd in opened:
        with pytest.raises(OSError):real_stat(fd)


def test_surrogate_path_rejects_before_any_filesystem_acquisition(tmp_path,monkeypatch):
    def forbidden(*a,**k):raise AssertionError('invalid Unicode path opened')
    monkeypatch.setattr(os,'open',forbidden)
    with pytest.raises(ArchiveError,match='Unicode'):owner.HeldInputFile(tmp_path/'\ud800',8)


def test_cleanup_preserves_prior_diagnostics_and_original_exception_cause(tmp_path,monkeypatch):
    path=tmp_path/'input';path.write_bytes(b'original')
    instance=owner.HeldInputFile(path,8);fd=instance.descriptor;real=os.close
    original=KeyboardInterrupt('original interruption');cause=RuntimeError('existing cause')
    prior=OSError('earlier exact cleanup error');later=OSError('later ambiguous close')
    original.__cause__=cause;original.cleanup_errors=(prior,)
    def closing(number):
        real(number)
        if number==fd:raise later
    monkeypatch.setattr(os,'close',closing)
    owner.cleanup_preserving(original,instance)
    assert original.__cause__ is cause and original.cleanup_errors[0] is prior
    assert original.cleanup_errors[1].cleanup_errors==(later,)
    instance.close()
