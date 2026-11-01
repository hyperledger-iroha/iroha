"""Independent bridge record/hash controls using actual compiled DLL/SO originals."""
from contextlib import ExitStack
from dataclasses import replace
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0,str(Path(__file__).parent))
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'scripts'))
from test_release_process_bridge import compiled
import release_bridge_admission as admitted_bridge
import release_entry_owner as entry
import workspace_source_keeper as custody
import compute_workspace_source_manifest as source_module
from test_release_entry_owner import Fixture as EntryFixture, ENVIRONMENT_POLICY, NATIVE_MODULE


def digest(value):return hashlib.sha256(value).hexdigest()


class Fixture:
    def __init__(self,root,compiled):
        program,library=compiled
        self.entry=EntryFixture(root/'entry-inputs',program.read_bytes())
        self.root=root/'bridge-parent'/'bridge';self.root.mkdir(parents=True)
        self.library=self.root/admitted_bridge.filename()
        shutil.copyfile(library,self.library)
        if os.name=='posix':self.library.chmod(0o500)
        self.value=dict(self.entry.value)
        self.value.update(executable_filename=admitted_bridge.filename(),
            executable_sha256=digest(self.library.read_bytes()),executable_bytes=self.library.stat().st_size)
        self.raw=(json.dumps(self.value,sort_keys=True,separators=(',',':'))+'\n').encode()
        (self.root/'publisher-source.json').write_bytes(self.raw)
        self.pins=admitted_bridge.BridgePins(self.value['executable_sha256'],digest(self.raw),
            self.entry.aggregate,self.entry.target)

    def load(self,owners,pins=None):
        source=owners.enter_context(source_module.retained_workspace_source(self.entry.source,
            self.entry.aggregate,custody_module=custody,native_module=NATIVE_MODULE))
        return admitted_bridge.load_original(self.root,pins or self.pins,source,owners,
            entry_module=entry,custody_module=custody,environment_policy=ENVIRONMENT_POLICY,native_module=NATIVE_MODULE)


def test_independently_pinned_library_loads_only_inside_its_original_owner(tmp_path,compiled):
    fixture=Fixture(tmp_path,compiled)
    with ExitStack() as owners:
        bridge=fixture.load(owners)
        bridge.validate()
        settings={'program':str(fixture.entry.path),'directory':str(tmp_path),
            'arguments':['echo',str(tmp_path/'release'),str(tmp_path/'finished')],
            'environment':{key:value for key,value in os.environ.items() if key.upper()=='SYSTEMROOT'}}
        assert bridge.run(settings,owners)==b'owned-native-entry\n'
        assert (tmp_path/'finished').is_file()
    with pytest.raises((RuntimeError,OSError)):
        bridge.validate()


@pytest.mark.parametrize('field',['executable_sha256','provenance_sha256','source_manifest_sha256'])
def test_independent_bridge_pin_mismatch_refuses_before_library_load(tmp_path,compiled,monkeypatch,field):
    fixture=Fixture(tmp_path,compiled)
    called=[]
    def foreign_load(*args,**kwargs):called.append(True);raise AssertionError('untrusted library load')
    monkeypatch.setattr(admitted_bridge.ctypes,'CDLL',foreign_load)
    with ExitStack() as owners:
        with pytest.raises((RuntimeError,OSError)):
            fixture.load(owners,replace(fixture.pins,**{field:'0'*64}))
    assert called==[]


def test_unpinned_sibling_is_not_allowed_to_supply_loader_dependencies(tmp_path,compiled,monkeypatch):
    fixture=Fixture(tmp_path,compiled)
    (fixture.root/'vcruntime140.dll').write_bytes(b'not independently admitted')
    with ExitStack() as owners:
        with pytest.raises(RuntimeError,match='exact two-entry'):
            fixture.load(owners)


def test_bridge_cannot_execute_after_its_caller_scope_released_originals(tmp_path,compiled):
    fixture=Fixture(tmp_path,compiled)
    with ExitStack() as owners:
        bridge=fixture.load(owners)
    with pytest.raises((RuntimeError,OSError)):
        bridge.run({},owners)


def test_bridge_cannot_transfer_a_different_empty_stack_as_custody(tmp_path,compiled):
    fixture=Fixture(tmp_path,compiled)
    with ExitStack() as owners:
        bridge=fixture.load(owners)
        with ExitStack() as empty:
            with pytest.raises(RuntimeError,match='cannot start'):
                bridge.run({},empty)
