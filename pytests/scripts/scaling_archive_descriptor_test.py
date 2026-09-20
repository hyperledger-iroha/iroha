"""Actual archive read/constructor descriptor ownership; no child or native code."""
import fcntl
import os
from pathlib import Path
import sys

import pytest
ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'scripts/nexus'),str(ROOT/'scripts')]
import scaling_archive_data as m

@pytest.fixture
def scope(tmp_path):
    root=tmp_path.resolve()/'archive';root.mkdir(mode=0o700)
    nested=root/'nested';nested.mkdir(mode=0o700)
    for path in (root/'input',nested/'input'):
        path.write_bytes(b'original');path.chmod(0o600)
    foreign=tmp_path.resolve()/'foreign';foreign.write_bytes(b'foreign');foreign.chmod(0o600)
    trees=[];replacements=[]
    def tree():
        owner=m._Tree(root,{'input':64,'nested/input':64},128);trees.append(owner);return owner
    def swap(fd,path,flags=os.O_RDONLY,inheritable=False):
        other=os.open(path,flags|os.O_CLOEXEC)
        os.dup2(other,fd,inheritable=inheritable);os.close(other);replacements.append(fd)
    yield root,foreign,tree,swap,replacements
    for owner in reversed(trees):
        try:owner.close()
        except OSError:pass
    for fd in replacements:
        try:os.close(fd)
        except OSError:pass


def test_original_read_scan_and_close(scope):
    root,foreign,create,swap,slots=scope;tree=create()
    assert tree.read('input',retain=True)==b'original'
    assert tree.scan().files==2
    fds=[row[0] for row in tree.chain]+[fd for name,fd in tree.directories.items() if name]
    tree.close();tree.close()
    for fd in fds:
        with pytest.raises(OSError):os.fstat(fd)


@pytest.mark.parametrize('same_inode',[False,True])
def test_read_error_replacement_is_preserved(scope,monkeypatch,same_inode):
    root,foreign,create,swap,slots=scope;tree=create();seen=[]
    def fail(fd,count):
        seen.append(fd);swap(fd,root/'input' if same_inode else foreign,
            os.O_WRONLY if same_inode else os.O_RDONLY)
        raise OSError('read failed')
    with monkeypatch.context() as patch:
        patch.setattr(os,'read',fail)
        with pytest.raises(OSError):tree.read('input',retain=True)
    os.fstat(seen[0])


@pytest.mark.parametrize('field',['inheritance','nonblocking'])
def test_same_inode_read_flag_drift_rejected_and_preserved(scope,monkeypatch,field):
    root,foreign,create,swap,slots=scope;tree=create();read=os.read;seen=[]
    def drift(fd,count):
        raw=read(fd,count)
        if not seen:
            seen.append(fd);slots.append(fd)
            if field=='inheritance':os.set_inheritable(fd,True)
            else:fcntl.fcntl(fd,fcntl.F_SETFL,fcntl.fcntl(fd,fcntl.F_GETFL)^os.O_NONBLOCK)
        return raw
    with monkeypatch.context() as patch:
        patch.setattr(os,'read',drift)
        with pytest.raises(m.ArchiveDataError):tree.read('input',retain=True)
    os.fstat(seen[0])


@pytest.mark.parametrize('relative',['','nested'])
def test_reused_directory_slot_preserved_by_close(scope,relative):
    root,foreign,create,swap,slots=scope;tree=create();fd=tree.directories[relative]
    swap(fd,foreign)
    tree.close()
    assert os.pread(fd,7,0)==b'foreign'


@pytest.mark.parametrize('relative',['','nested'])
@pytest.mark.parametrize('field',['inheritance','nonblocking'])
def test_same_inode_directory_flags_are_checked_and_preserved(scope,relative,field):
    root,foreign,create,swap,slots=scope;tree=create();fd=tree.directories[relative]
    if field=='inheritance':swap(fd,root/relative,os.O_RDONLY|os.O_DIRECTORY,True)
    else:
        slots.append(fd);fcntl.fcntl(fd,fcntl.F_SETFL,fcntl.fcntl(fd,fcntl.F_GETFL)^os.O_NONBLOCK)
    try:
        with pytest.raises(m.ArchiveDataError):tree.check()
    finally:tree.close()
    os.fstat(fd)


def test_ancestor_slot_replacement_preserved(scope):
    root,foreign,create,swap,slots=scope;tree=create();fd=tree.chain[-2][0]
    swap(fd,foreign)
    tree.close();assert os.read(fd,7)==b'foreign'


def test_constructor_nested_error_preserves_reused_slot(scope,monkeypatch):
    root,foreign,create,swap,slots=scope;actual=m._Tree._open;seen=[]
    def opened(owner,relative,fd):
        if relative=='nested':
            seen.append(fd);swap(fd,foreign);raise OSError('nested scan failed')
        return actual(owner,relative,fd)
    with monkeypatch.context() as patch:
        patch.setattr(m._Tree,'_open',opened)
        with pytest.raises(OSError):create()
    assert os.read(seen[0],7)==b'foreign'


def test_constructor_chain_error_preserves_reused_slot(scope,monkeypatch):
    root,foreign,create,swap,slots=scope;actual_open=os.open;actual_stat=os.stat;seen=[]
    def opened(path,flags,*args,**kwargs):
        fd=actual_open(path,flags,*args,**kwargs)
        if str(path)==root.name:seen.append(fd)
        return fd
    def stated(path,*args,**kwargs):
        if str(path)==root.name and seen:
            swap(seen[-1],foreign);raise OSError('chain inspection failed')
        return actual_stat(path,*args,**kwargs)
    with monkeypatch.context() as patch:
        patch.setattr(os,'open',opened);patch.setattr(os,'stat',stated)
        with pytest.raises(OSError):create()
    assert os.pread(seen[-1],7,0)==b'foreign'


def test_constructor_error_closes_original_nested_and_chain_descriptors(scope,monkeypatch):
    root,foreign,create,swap,slots=scope;actual=m._Tree._open;seen=[]
    def opened(owner,relative,fd):
        if relative=='nested':
            seen.extend(row[0] for row in owner.chain);seen.append(fd)
            raise OSError('scan failed')
        return actual(owner,relative,fd)
    with monkeypatch.context() as patch:
        patch.setattr(m._Tree,'_open',opened)
        with pytest.raises(OSError):create()
    for fd in seen:
        with pytest.raises(OSError):os.fstat(fd)


def test_digest_materialization_drift_is_rejected_and_preserved(scope,monkeypatch):
    root,foreign,create,swap,slots=scope;tree=create();actual_open=os.open;sha=m.hashlib.sha256;seen=[]
    def opened(path,*args,**kwargs):
        fd=actual_open(path,*args,**kwargs)
        if str(path)=='input':seen.append(fd)
        return fd
    class Hash:
        def __init__(self):self.original=sha()
        def update(self,raw):self.original.update(raw)
        def hexdigest(self):
            result=self.original.hexdigest();swap(seen[-1],foreign);return result
    with monkeypatch.context() as patch:
        patch.setattr(os,'open',opened);patch.setattr(m.hashlib,'sha256',Hash)
        with pytest.raises(m.ArchiveDataError):tree.read('input')
    assert os.pread(seen[-1],7,0)==b'foreign'


def test_closed_tree_cannot_report_a_valid_scope(scope):
    root,foreign,create,swap,slots=scope;tree=create();tree.close()
    with pytest.raises(m.ArchiveDataError):tree.check()


def test_reuse_discovered_in_final_cleanup_rejects_result(scope,monkeypatch):
    root,foreign,create,swap,slots=scope;tree=create();seen=[];actual=m._close_original_descriptor
    def replaced(fd,pin):
        if not seen:
            seen.append(fd);swap(fd,foreign)
        return actual(fd,pin)
    with monkeypatch.context() as patch:
        patch.setattr(m,'_close_original_descriptor',replaced)
        with pytest.raises(m.ArchiveDataError):tree.read('input',retain=True)
    assert os.pread(seen[0],7,0)==b'foreign'


def test_closed_slot_does_not_prevent_remaining_original_cleanup(scope):
    root,foreign,create,swap,slots=scope;tree=create()
    fds=[row[0] for row in tree.chain]+[tree.directories['nested']]
    os.close(tree.directories['nested']);tree.close()
    for fd in fds:
        with pytest.raises(OSError):os.fstat(fd)


def test_closed_tree_read_cannot_reopen_a_scope(scope):
    root,foreign,create,swap,slots=scope;tree=create();tree.close()
    with pytest.raises(m.ArchiveDataError):tree.read('input')
