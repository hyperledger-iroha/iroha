"""Original primary descriptor controls; no SDK execution qualification."""
from __future__ import annotations

import os
import io
from pathlib import Path
import sys
import subprocess
import time
import zipfile

import pytest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
import sorafs_python_producer_inputs as inputs
from sorafs_python_archive import execution_archive


def test_execution_archive_is_order_independent_and_retains_empty_pip_metadata():
    members = {"logs/stdout": b"actual output\n", "installed/REQUESTED": b""}
    raw = execution_archive(members)
    assert raw == execution_archive(dict(reversed(tuple(members.items()))))
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        assert archive.namelist() == sorted(members)
        assert {name: archive.read(name) for name in archive.namelist()} == members
        assert all(info.date_time == (1980, 1, 1, 0, 0, 0) for info in archive.infolist())


@pytest.mark.parametrize("name", ("../escape", "/absolute", "a//b", "a\\b", ""))
def test_execution_archive_refuses_unsafe_member_paths(name):
    with pytest.raises(ValueError): execution_archive({name: b"original"})


def test_fresh_outputs_never_replace_original_files(tmp_path):
    path = tmp_path / "artifact"
    inputs.write_fresh(path, b"original")
    with pytest.raises(FileExistsError): inputs.write_fresh(path, b"replacement")
    assert path.read_bytes() == b"original"


def test_original_held_descriptor_is_used_for_repeat_read_and_recheck(tmp_path,monkeypatch):
    path=tmp_path/'original'; path.write_bytes(b'input')
    with inputs.OriginalInputs() as owner:
        assert owner.read(path,5,hold=True)==b'input'
        descriptor=owner._held[path][0]
        assert os.fstat(descriptor).st_ino==path.stat().st_ino
        def forbidden(*_args,**_kwargs): raise AssertionError('held original reopened')
        monkeypatch.setattr(inputs.child,'read_stable',forbidden)
        assert owner.read(path,5)==b'input'
        owner.recheck()
    with pytest.raises(OSError): os.fstat(descriptor)


@pytest.mark.parametrize('mutation',('same_bytes_replace','different_bytes','chmod','grow','symlink','unlink'))
def test_held_original_mutations_fail(tmp_path,mutation):
    path=tmp_path/'input'; path.write_bytes(b'old')
    with pytest.raises((inputs.ArtifactError,OSError)):
        with inputs.OriginalInputs() as owner:
            owner.read(path,8,hold=True)
            if mutation=='same_bytes_replace': path.unlink(); path.write_bytes(b'old')
            elif mutation=='different_bytes': path.write_bytes(b'new')
            elif mutation=='chmod': path.chmod(0o400)
            elif mutation=='grow': path.write_bytes(b'longer input')
            elif mutation=='unlink': path.unlink()
            else:
                other=tmp_path/'replacement'; other.write_bytes(b'old')
                path.unlink(); path.symlink_to(other)
            owner.recheck()
    assert owner._closed and not owner._held and not owner._parents


def test_replaced_ancestor_with_original_file_is_refused(tmp_path):
    parent=tmp_path/'parent'; parent.mkdir(); path=parent/'input'; path.write_bytes(b'original')
    with pytest.raises(inputs.ArtifactError,match='ancestor'):
        with inputs.OriginalInputs() as owner:
            owner.read(path,8,hold=True)
            old=tmp_path/'old-parent'; parent.rename(old); parent.mkdir()
            owner.recheck()


def test_fifo_rejects_without_blocking(tmp_path):
    path=tmp_path/'fifo'; os.mkfifo(path)
    started=time.monotonic()
    with inputs.OriginalInputs() as owner:
        with pytest.raises(inputs.ArtifactError,match='regular file'): owner.read(path,16,hold=True)
    assert time.monotonic()-started<1


def test_hardlink_is_not_admitted(tmp_path):
    path=tmp_path/'input'; path.write_bytes(b'input'); os.link(path,tmp_path/'alias')
    with inputs.OriginalInputs() as owner:
        with pytest.raises(inputs.ArtifactError,match='single-link'): owner.read(path,5,hold=True)


def test_actual_256_primary_ceiling_and_cleanup(tmp_path):
    owner=inputs.OriginalInputs()
    with owner:
        for index in range(256):
            path=tmp_path/str(index); path.write_bytes(b'x'); owner.read(path,1,hold=True)
        assert len(owner._held)==256
        overflow=tmp_path/'overflow'; overflow.write_bytes(b'x')
        with pytest.raises(inputs.ArtifactError,match='file count'): owner.read(overflow,1,hold=True)
        descriptors=[fd for fd,_ in owner._held.values()]
    for descriptor in descriptors:
        with pytest.raises(OSError): os.fstat(descriptor)


def test_primary_aggregate_bound_uses_actual_sizes(tmp_path,monkeypatch):
    monkeypatch.setattr(inputs.OriginalInputs,'MAX_HELD_BYTES',5)
    with inputs.OriginalInputs() as owner:
        first=tmp_path/'first'; first.write_bytes(b'12345'); owner.read(first,5,hold=True)
        second=tmp_path/'second'; second.write_bytes(b'6')
        with pytest.raises(inputs.ArtifactError,match='bounded'): owner.read(second,1,hold=True)
        assert len(owner._held)==1


def test_streamed_input_can_be_promoted_without_changing_identity(tmp_path):
    path=tmp_path/'input'; path.write_bytes(b'data')
    with inputs.OriginalInputs() as owner:
        assert owner.read(path,4)==b'data' and not owner._held
        seal=owner.files[path]
        assert owner.read(path,4,hold=True)==b'data' and owner.files[path]==seal


def test_close_is_idempotent_but_owner_cannot_be_reused(tmp_path):
    path=tmp_path/'input'; path.write_bytes(b'data')
    owner=inputs.OriginalInputs(); owner.read(path,4,hold=True); owner.close(); owner.close()
    for operation in (lambda:owner.read(path,4),owner.recheck,owner.__enter__):
        with pytest.raises(inputs.ArtifactError): operation()


def test_exception_exit_closes_original_descriptors(tmp_path):
    path=tmp_path/'input'; path.write_bytes(b'data')
    with pytest.raises(RuntimeError,match='caller failure'):
        with inputs.OriginalInputs() as owner:
            owner.read(path,4,hold=True); descriptor=owner._held[path][0]
            raise RuntimeError('caller failure')
    with pytest.raises(OSError): os.fstat(descriptor)


@pytest.mark.parametrize('maximum,hold',((True,True),(-1,True),(inputs.OriginalInputs.MAX_FILE_BYTES+1,True),(4,1)))
def test_read_argument_bounds_fail_before_open(tmp_path,maximum,hold):
    path=tmp_path/'input'; path.write_bytes(b'data')
    with inputs.OriginalInputs() as owner:
        with pytest.raises(inputs.ArtifactError): owner.read(path,maximum,hold=hold)
        assert not owner._held and not owner._parents


def test_empty_primary_and_smaller_repeat_bound(tmp_path):
    empty=tmp_path/'empty'; empty.write_bytes(b'')
    path=tmp_path/'data'; path.write_bytes(b'data')
    with inputs.OriginalInputs() as owner:
        assert owner.read(empty,0,hold=True)==b''
        owner.read(path,4,hold=True)
        with pytest.raises(inputs.ArtifactError,match='byte bound'): owner.read(path,3)
        assert owner.read(path,4)==b'data'


def test_complete_tree_streams_and_detects_later_file_change(tmp_path):
    root=tmp_path/'tree'; root.mkdir(); (root/'nested').mkdir()
    (root/'a').write_bytes(b'a'); (root/'nested/b').write_bytes(b'b')
    with pytest.raises((inputs.ArtifactError,inputs.child.QualificationError)):
        with inputs.OriginalInputs() as owner:
            captured=inputs.capture_tree(root,owner,maximum=2,maximum_file=1,maximum_entries=3)
            assert captured=={'a':b'a','nested/b':b'b'} and not owner._held
            (root/'a').write_bytes(b'changed')
            owner.recheck()



def test_unchanged_original_wheel_harness_owns_native_and_installed_join_controls(tmp_path):
    source = r"""
import importlib.util,sys
from pathlib import Path
from types import SimpleNamespace
from dataclasses import replace
root,fixture=map(Path,sys.argv[1:])
sys.path.insert(0,str(root/'scripts'))
import sorafs_python_producer_inputs as inputs
from sorafs_python_archive import execution_archive
spec=importlib.util.spec_from_file_location('original_byte_controls',root/'scripts/tests/python_wheel_byte_owner_test.py')
controls=importlib.util.module_from_spec(spec); spec.loader.exec_module(controls)
original={'__name__':'__main__','__file__':str(controls.SHELL_HARNESS)}
sys.argv=[str(controls.SHELL_HARNESS),str(root/'ci/verify_privacy_python_wheel.py'),str(fixture)]
exec(compile(controls.extract_original_harness(controls.SHELL_HARNESS.read_bytes()),str(controls.SHELL_HARNESS),'exec'),original)
verifier=inputs.verifier
path=original['write_wheel'](fixture/'join-native.whl',original['valid_entries']())
wheel=verifier.preflight_wheel(path,original['seal'](path),extension_suffixes=('.abi3.so',))
assert inputs.native_member(path.read_bytes(),wheel)==original['native_bytes']
sdk=verifier.parse_wheel_bytes(original['sdk_path'].read_bytes(),owner=verifier.SDK_OWNER)
try: inputs.native_member(original['sdk_path'].read_bytes(),sdk)
except inputs.ArtifactError: pass
else: raise AssertionError('SDK acquired native extraction ownership')
original['install_dist_info'](wheel)
layout=verifier.derive_installed_layout(environment_root=original['environment_root'],site_roots={original['site_root']},wheel=wheel)
installed=verifier.verify_installed_files(wheel,layout)
rows=tuple(SimpleNamespace(path=str(value.path),seal=value.seal) for value in installed.files)
observation=SimpleNamespace(owner=wheel.owner.package,version=wheel.metadata_version,path=str(wheel.path),seal=wheel.seal,installed_files=rows)
with inputs.OriginalInputs() as owner:
    retained=inputs.installed_wheel_join(observation,wheel,owner)
assert set(retained)=={wheel.dist_info_root+'/RECORD',wheel.dist_info_root+'/direct_url.json'}
controls=0
for change in ('version','owner','path','seal','missing','installed_seal'):
    changed=SimpleNamespace(**vars(observation))
    if change=='version': changed.version='8.8.8'
    elif change=='owner': changed.owner='foreign'
    elif change=='path': changed.path='/foreign.whl'
    elif change=='seal': changed.seal=replace(changed.seal,inode=changed.seal.inode+1)
    elif change=='missing': changed.installed_files=rows[:-1]
    else: changed.installed_files=(*rows[:-1],SimpleNamespace(path=rows[-1].path,seal=replace(rows[-1].seal,size=rows[-1].seal.size+1)))
    try:
        with inputs.OriginalInputs() as owner: inputs.installed_wheel_join(changed,wheel,owner)
    except inputs.ArtifactError: controls+=1
    else: raise AssertionError('altered installed observation admitted: '+change)
assert controls==6
print('PRODUCER_JOIN_CONTROLS=6')
"""
    result=subprocess.run((sys.executable,'-I','-B','-c',source,str(ROOT),str(tmp_path/'fixture')),
                          capture_output=True,text=True,timeout=120,check=False)
    (tmp_path/'original-harness.log').write_text(result.stdout+result.stderr)
    assert result.returncode==0,result.stdout+result.stderr
    assert 'two-wheel bounded archive, installed-origin, loader, missing-owner and tamper checks passed' in result.stdout
    assert 'PRODUCER_JOIN_CONTROLS=6' in result.stdout
