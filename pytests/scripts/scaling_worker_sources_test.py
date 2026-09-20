"""Fixed real-file source custody and one explicit Python -B -S import smoke.

No Iroha binary, network, native build, quota or process signal is exercised.
The sole child uses the existing Python interpreter and returns before worker
configuration or process observation; it proves local import compatibility only.
"""
import ast
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

import pytest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/nexus'))
import scaling_worker_sources as sources


def pack(tmp_path):
    root=tmp_path.resolve()/'worker-sources';root.mkdir(mode=0o700)
    pins=[]
    for name in sources.SOURCE_NAMES:
        raw=(ROOT/'scripts/nexus'/name).read_bytes()
        path=root/name;path.write_bytes(raw);path.chmod(0o600)
        pins.append(sources.WorkerSourcePin(name,hashlib.sha256(raw).hexdigest(),len(raw)))
    return root,tuple(pins)


@pytest.fixture
def case(tmp_path):
    root,pins=pack(tmp_path);owner=sources.WorkerSourceFiles(root,pins)
    yield root,pins,owner
    owner.close()


def test_exact_original_source_pack_and_bounded_lifetime(case):
    root,pins,owner=case
    assert owner.directory==root and owner.worker_path==root/sources.SOURCE_NAMES[0]
    assert owner.pins is pins and len(owner._files)==5
    assert len(owner._chain)<=sources.MAX_ANCESTORS
    assert len(owner._handles)==len(owner._chain)+5
    assert sum(pin.bytes for pin in pins)<=sources.MAX_TOTAL_SOURCE_BYTES
    with owner as same:assert same is owner
    assert all((root/name).is_file() for name in sources.SOURCE_NAMES)
    with pytest.raises(sources.WorkerSourceError):owner.validate()


@pytest.mark.parametrize('kind',['missing','extra','pycache','shadow_stdlib','directory','symlink','hardlink','mode','rootmode','hash','size','oversize'])
def test_initial_source_census_identity_and_bounds_reject_without_fd_leaks(tmp_path,kind):
    root,pins=pack(tmp_path);first=root/sources.SOURCE_NAMES[0]
    if kind=='missing':first.unlink()
    elif kind=='extra':(root/'other.py').write_text('')
    elif kind=='pycache':(root/'__pycache__').mkdir()
    elif kind=='shadow_stdlib':(root/'json.py').write_text('raise RuntimeError()')
    elif kind=='directory':first.unlink();first.mkdir(mode=0o700)
    elif kind=='symlink':
        detached=root.parent/'detached';first.rename(detached);first.symlink_to(detached)
    elif kind=='hardlink':os.link(first,root.parent/'other-link')
    elif kind=='mode':first.chmod(0o644)
    elif kind=='rootmode':root.chmod(0o755)
    elif kind=='hash':pins=(replace(pins[0],sha256='0'*64),*pins[1:])
    elif kind=='size':pins=(replace(pins[0],bytes=pins[0].bytes+1),*pins[1:])
    else:pins=(replace(pins[0],bytes=sources.MAX_SOURCE_BYTES+1),*pins[1:])
    before=set(os.listdir('/dev/fd'))
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,pins)
    assert set(os.listdir('/dev/fd'))==before


@pytest.mark.parametrize('kind',['rewrite','replace','remove','mode','root','ancestor','extra','pin'])
def test_original_source_changes_are_terminal_and_close_preserves_files(case,kind):
    root,pins,owner=case;first=root/sources.SOURCE_NAMES[0]
    if kind=='rewrite':first.write_bytes(first.read_bytes())
    elif kind=='replace':
        raw=first.read_bytes();first.rename(root.parent/'detached');first.write_bytes(raw);first.chmod(0o600)
    elif kind=='remove':first.unlink()
    elif kind=='mode':first.chmod(0o644)
    elif kind=='root':root.rename(root.with_name('detached'));root.mkdir(mode=0o700)
    elif kind=='ancestor':
        parent=root.parent;parent.rename(parent.with_name(parent.name+'-detached'));parent.mkdir(mode=0o700)
    elif kind=='extra':(root/'json.py').write_text('')
    else:object.__setattr__(pins[0],'sha256','0'*64)
    with pytest.raises(sources.WorkerSourceError):owner.validate()
    with pytest.raises(sources.WorkerSourceError):_=owner.worker_path
    owner.close()
    assert owner._handles==[]


@pytest.mark.parametrize('kind',['replace','rewrite'])
def test_earlier_source_mutation_during_later_initial_hash_is_rejected(tmp_path,monkeypatch,kind):
    root,pins=pack(tmp_path);first=root/sources.SOURCE_NAMES[0];last=root/sources.SOURCE_NAMES[-1]
    last_inode=last.stat().st_ino;original=os.pread;mutated=[]
    def read(fd,length,offset):
        raw=original(fd,length,offset)
        if os.fstat(fd).st_ino==last_inode and not mutated:
            mutated.append(True);before=first.read_bytes()
            if kind=='replace':first.rename(root.parent/'detached')
            first.write_bytes(before);first.chmod(0o600)
        return raw
    monkeypatch.setattr(os,'pread',read);before=set(os.listdir('/dev/fd'))
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,pins)
    assert mutated and set(os.listdir('/dev/fd'))==before


def test_short_initial_read_cannot_admit_truncated_source(tmp_path,monkeypatch):
    root,pins=pack(tmp_path);monkeypatch.setattr(os,'pread',lambda *_:b'')
    before=set(os.listdir('/dev/fd'))
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,pins)
    assert set(os.listdir('/dev/fd'))==before


def test_source_names_require_original_plain_strings(tmp_path):
    root,pins=pack(tmp_path)
    class Name(str):pass
    changed=(replace(pins[0],name=Name(pins[0].name)),*pins[1:])
    before=set(os.listdir('/dev/fd'))
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,changed)
    assert set(os.listdir('/dev/fd'))==before


def test_foreign_pin_name_is_rejected_before_equality_or_path_open(tmp_path,monkeypatch):
    root,pins=pack(tmp_path);calls=[]
    class Name:
        def __eq__(self,other):calls.append('equality');return True
        def __fspath__(self):calls.append('path');return 'foreign.py'
    changed=(replace(pins[0],name=Name()),*pins[1:])
    def opened(*_,**__):calls.append('open');raise AssertionError('unadmitted open')
    monkeypatch.setattr(os,'open',opened)
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,changed)
    assert calls==[]


@pytest.mark.parametrize('field',['name','sha256','bytes'])
def test_admitted_pin_types_cannot_forge_equality(case,field):
    root,pins,owner=case;calls=[]
    class Text(str):
        def __eq__(self,other):calls.append('equality');return True
    class Number(int):
        def __eq__(self,other):calls.append('equality');return True
    value=Number(pins[0].bytes+1) if field=='bytes' else Text('foreign')
    object.__setattr__(pins[0],field,value)
    with pytest.raises(sources.WorkerSourceError):_=owner.pins
    assert calls==[]
    with pytest.raises(sources.WorkerSourceError):owner.validate()


def test_admitted_pin_objects_cannot_be_replaced_by_subclasses(case):
    root,pins,owner=case
    class Pin(sources.WorkerSourcePin):pass
    owner._pins=(Pin(pins[0].name,pins[0].sha256,pins[0].bytes),*pins[1:])
    with pytest.raises(sources.WorkerSourceError):_=owner.pins


def test_admitted_pin_tuple_cannot_be_replaced_by_a_list(case):
    root,pins,owner=case;owner._pins=list(pins)
    with pytest.raises(sources.WorkerSourceError):_=owner.pins


@pytest.mark.parametrize('phase',['initial','validate'])
def test_final_namespace_scan_cannot_rewrite_an_earlier_source(tmp_path,monkeypatch,phase):
    root,pins=pack(tmp_path);first=root/sources.SOURCE_NAMES[0]
    owner=sources.WorkerSourceFiles(root,pins) if phase=='validate' else None
    last_inode=(root/sources.SOURCE_NAMES[-1]).stat().st_ino
    original_scan=os.scandir;original_read=os.pread;scan_calls=[];hashed=[phase=='validate'];mutated=[]
    def read(fd,size,offset):
        raw=original_read(fd,size,offset)
        if os.fstat(fd).st_ino==last_inode:hashed[0]=True
        return raw
    class Scan:
        def __init__(self,fd):self.entries=original_scan(fd)
        def __enter__(self):return self.entries.__enter__()
        def __exit__(self,*args):
            result=self.entries.__exit__(*args)
            if hashed[0]:
                scan_calls.append(True)
                if len(scan_calls)==2:
                    first.write_bytes(first.read_bytes());mutated.append(True)
            return result
    monkeypatch.setattr(os,'pread',read);monkeypatch.setattr(os,'scandir',Scan)
    before=set(os.listdir('/dev/fd'))
    try:
        with pytest.raises(sources.WorkerSourceError):
            if owner is None:sources.WorkerSourceFiles(root,pins)
            else:owner.validate()
        assert mutated and set(os.listdir('/dev/fd'))==before
    finally:
        if owner is not None:owner.close()


@pytest.mark.parametrize('access',['validate','directory','worker_path','pins'])
def test_original_returned_path_cannot_be_retargeted(case,access):
    root,pins,owner=case
    owner._directory=root.with_name('foreign-sources')
    with pytest.raises(sources.WorkerSourceError):
        if access=='validate':owner.validate()
        else:getattr(owner,access)


def test_close_never_closes_a_reused_foreign_descriptor(case):
    root,pins,owner=case;fd=owner._files[0][0];retained=os.dup(fd)
    foreign=os.open(root/sources.SOURCE_NAMES[1],os.O_RDONLY)
    try:
        os.dup2(foreign,fd)
        with pytest.raises(sources.WorkerSourceError):owner.close()
        assert os.fstat(fd).st_ino==os.fstat(foreign).st_ino and len(owner._handles)==1
        os.dup2(retained,fd);owner.close()
    finally:os.close(retained);os.close(foreign)


@pytest.mark.parametrize('kind',['order','duplicate','bool','relative','parent','symlink_ancestor'])
def test_canonical_original_admission_has_no_path_or_pin_aliases(tmp_path,kind):
    root,pins=pack(tmp_path)
    if kind=='order':pins=tuple(reversed(pins))
    elif kind=='duplicate':pins=(pins[0],pins[0],*pins[2:])
    elif kind=='bool':pins=(replace(pins[0],bytes=True),*pins[1:])
    elif kind=='relative':root=Path('worker-sources')
    elif kind=='parent':root=root/'..'/'worker-sources'
    else:
        alias=root.parent/'alias';alias.symlink_to(root);root=alias
    before=set(os.listdir('/dev/fd'))
    with pytest.raises(sources.WorkerSourceError):sources.WorkerSourceFiles(root,pins)
    assert set(os.listdir('/dev/fd'))==before


def test_actual_five_source_local_import_graph_is_closed(case):
    root,_,owner=case;local={Path(name).stem for name in sources.SOURCE_NAMES};edges={}
    for name in sources.SOURCE_NAMES:
        tree=ast.parse((root/name).read_text());imports=set()
        for node in ast.walk(tree):
            if isinstance(node,ast.Import):imports.update(alias.name.split('.')[0] for alias in node.names)
            elif isinstance(node,ast.ImportFrom):
                assert node.level==0 and node.module is not None
                imports.add(node.module.split('.')[0])
        assert imports<=local|sys.stdlib_module_names
        edges[Path(name).stem]=imports&local
    assert edges['resource_probe_worker']=={'resource_probe','resource_process','resource_evidence_budget'}
    assert edges['resource_probe']=={'resource_process','resource_evidence_budget','kura_resource_metrics'}
    assert not edges['resource_process'] and not edges['resource_evidence_budget'] and not edges['kura_resource_metrics']
    owner.validate()


def test_fixed_python_flags_import_actual_five_sources_without_site_or_bytecode_writes(case,tmp_path):
    root,_,owner=case
    stdout=tmp_path/'stdout';stderr=tmp_path/'stderr'
    argv=(sys.executable,'-B','-S',str(owner.worker_path))
    with stdout.open('xb') as out,stderr.open('xb') as err:
        child=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=out,stderr=err,cwd='/',env={},close_fds=True)
        assert child.wait(timeout=20)==2  # Actual worker main rejects missing fixed config arguments after imports.
    assert stdout.stat().st_size==0 and stderr.stat().st_size==0
    assert set(path.name for path in root.iterdir())==set(sources.SOURCE_NAMES)
    owner.validate()
