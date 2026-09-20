"""Real portable package files and fresh admission, with explicit import seam.

No child process, native command, original execution attribution or portable
trust anchor is supplied by these tests. The existing package import method is
replaced only after its real fresh owner and file inventory are established.
"""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
from types import SimpleNamespace as NS
import pytest
ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'scripts/nexus'),str(ROOT/'scripts')]
import scaling_cli_bootstrap as api
import scaling_release_record as record_api

def sha(raw):return hashlib.sha256(raw).hexdigest()
def encode(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode('ascii')
def envelope(binding):
    # This fixture exercises authenticated envelope framing only; the empty
    # semantic fields cannot qualify a full parent execution record.
    return encode(dict(schema=api.PARENT_EXECUTION_SCHEMA,candidate={},inputs={},execution={},publication={},verifier_python=binding,preflight={}))

@pytest.fixture(autouse=True)
def no_process(monkeypatch):
    def denied(*a,**k):raise AssertionError('child and signal operations forbidden')
    monkeypatch.setattr(subprocess,'Popen',denied)
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name):monkeypatch.setattr(os,name,denied)

@pytest.fixture(scope='module')
def retained(tmp_path_factory):
    root=tmp_path_factory.mktemp('retained-verifier');root.chmod(0o700)
    installed=Path(__import__('blake3').__file__).parent.parent
    api.stage_dependency_source(installed,root/'source')
    owner=api.PythonDependencies.provision(root/'source',root/'bundle',root/'inventory.json')
    try:
        binding=dict(inventory_sha256=owner.paths.inventory_sha256,files=list(api.dependency_package_census(root/'source')))
        assert api.dependency_package_census(root/'bundle')==tuple(binding['files'])
        owner.verify()
    finally:owner.close()
    return root,binding,envelope(binding)

@pytest.fixture
def copy(retained,tmp_path):
    root,binding,raw=retained
    copied=tmp_path/'relocated';shutil.copytree(root,copied);copied.chmod(0o700)
    return copied,binding,raw

def test_closed_envelope_and_census_ownership(retained):
    root,binding,raw=retained
    decoded=api.parent_execution_dependency_binding(raw,sha(raw))
    assert decoded==binding
    decoded['files'][0]['sha256']='0'*64
    assert api.parent_execution_dependency_binding(raw,sha(raw))==binding
    assert len(binding['files'])==8 and all(set(row)=={'path','size_bytes','sha256'} for row in binding['files'])
    assert api.MAX_PARENT_EXECUTION_BYTES==record_api.MAX_RECORD_BYTES==2*1024*1024+128*1024

@pytest.mark.parametrize('mutation',['digest','duplicate','depth','oversize','unknown','schema','float','bool','path','order','inventory','extra_row','cap','missing_preflight','preflight_type'])
def test_envelope_rejects_before_package_load(retained,mutation):
    _,binding,raw=retained;value=json.loads(raw)
    if mutation=='digest':
        with pytest.raises(ValueError):api.parent_execution_dependency_binding(raw,'0'*64)
        return
    if mutation=='duplicate':raw=raw.replace(b'{',b'{"schema":"wrong",',1)
    elif mutation=='depth':raw=b'['*65+b'0'+b']'*65
    elif mutation=='oversize':raw=b'x'*(api.MAX_PARENT_EXECUTION_BYTES+1)
    else:
        if mutation=='unknown':value['authority']=True
        elif mutation=='schema':value['schema']='retired'
        elif mutation=='float':value['verifier_python']['files'][0]['size_bytes']=1.0
        elif mutation=='bool':value['verifier_python']['files'][0]['size_bytes']=True
        elif mutation=='path':value['verifier_python']['files'][0]['path']='../elsewhere'
        elif mutation=='order':value['verifier_python']['files'].reverse()
        elif mutation=='inventory':value['verifier_python']['inventory_sha256']='no'
        elif mutation=='extra_row':value['verifier_python']['files'].append(binding['files'][0])
        elif mutation=='cap':value['verifier_python']['files'][0]['size_bytes']=api._MAX_PACKAGE_FILE+1
        elif mutation=='missing_preflight':del value['preflight']
        elif mutation=='preflight_type':value['preflight']=[]
        raw=encode(value)
    with pytest.raises(ValueError):api.parent_execution_dependency_binding(raw,sha(raw))

@pytest.mark.parametrize('relocated',[False,True])
def test_original_and_relocated_bytes_get_new_actual_inventory(retained,copy,tmp_path,monkeypatch,relocated):
    root,binding,raw=copy if relocated else retained
    calls=[]
    def load(owner):
        owner.verify();calls.append(owner.paths)
        assert sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode
        return NS(explicit_import_seam=True)
    monkeypatch.setattr(api.PythonDependencies,'load',load)
    owner=api.prepare_record_verifier(raw,sha(raw),root,tmp_path)
    try:
        owner.verify();assert len(calls)==1
        fresh=calls[0]
        assert fresh.source_root!=root/'source' and fresh.inventory!=root/'inventory.json'
        assert fresh.inventory_sha256!=binding['inventory_sha256']
        assert (fresh.source_root/'blake3/__init__.py').stat().st_ino!=(root/'source/blake3/__init__.py').stat().st_ino
        assert api.dependency_package_census(fresh.source_root)==tuple(binding['files'])
        assert api.dependency_package_census(fresh.bundle_root)==tuple(binding['files'])
    finally:owner.close()
    assert (root/'inventory.json').read_bytes()==retained[0].joinpath('inventory.json').read_bytes()
    with pytest.raises(ValueError):owner.verify()

@pytest.mark.parametrize('mutation',['inventory','source','bundle','extra','symlink','source_writable'])
def test_tampered_retained_package_never_reaches_import(copy,tmp_path,monkeypatch,mutation):
    root,_,raw=copy;calls=[]
    monkeypatch.setattr(api.PythonDependencies,'load',lambda owner:calls.append(owner))
    path=root/'source/blake3/__init__.py'
    if mutation=='inventory':path=root/'inventory.json';path.chmod(0o600);path.write_bytes(b'changed')
    elif mutation=='source':path.write_bytes(b'changed')
    elif mutation=='bundle':path=root/'bundle/blake3/__init__.py';path.chmod(0o600);path.write_bytes(b'changed')
    elif mutation=='extra':(root/'source/extra.py').write_bytes(b'changed')
    elif mutation=='symlink':path.unlink();path.symlink_to(root/'bundle/blake3/__init__.py')
    else:path.chmod(0o666)
    with pytest.raises(ValueError):api.prepare_record_verifier(raw,sha(raw),root,tmp_path)
    assert calls==[]

def test_retained_mutation_and_scope_parent_are_rejected(copy,tmp_path,monkeypatch):
    root,_,raw=copy
    monkeypatch.setattr(api.PythonDependencies,'load',lambda owner:owner.verify())
    insecure=tmp_path/'insecure';insecure.mkdir(mode=0o755)
    with pytest.raises(ValueError):api.prepare_record_verifier(raw,sha(raw),root,insecure)
    owner=api.prepare_record_verifier(raw,sha(raw),root,tmp_path)
    try:
        path=root/'source/blake3/__init__.py';path.write_bytes(path.read_bytes()+b'\n')
        with pytest.raises(ValueError):owner.verify()
    finally:owner.close()
