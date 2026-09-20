"""Actual late package ownership; only the prepared/loader access seam is explicit."""
from __future__ import annotations

import ast
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import types

import pytest
import blake3
from sumeragi_v2_release_scaling_selection_test import bootstrap_definitions
from sumeragi_v2_release_scaling_main_test import put

ROOT = Path(__file__).resolve().parents[2]
# The fixed driver loads this package through its retained dependency owner
# before collection. Copy that admitted package into each original test scope.
DEPENDENCIES = Path(blake3.__file__).resolve().parent.parent


@pytest.fixture
def staging(tmp_path, monkeypatch):
    m=bootstrap_definitions();root=tmp_path/'sealed-source';root.mkdir(mode=0o700)
    parsed=ast.parse((ROOT/'scripts/nexus/scaling_cli_bootstrap.py').read_bytes())
    names=next(ast.literal_eval(node.value) for node in parsed.body
        if isinstance(node,ast.Assign) and len(node.targets)==1
        and isinstance(node.targets[0],ast.Name) and node.targets[0].id=='PYTHON_SOURCE_FILES')
    names=tuple(dict.fromkeys((*names,*m._SCALING_PARENT_EXTENSIONS)))
    for name in names:
        monkeypatch.delitem(sys.modules,Path(name).stem,raising=False)
        put(root/name,(ROOT/name).read_bytes())
    writer=types.ModuleType('scaling_staging_fixture_source_writer')
    writer.__file__=str(root/'scripts/compute_workspace_source_manifest.py')
    monkeypatch.setitem(sys.modules,writer.__name__,writer)
    exec(compile(Path(writer.__file__).read_bytes(),writer.__file__,'exec'),writer.__dict__)
    paths=tmp_path/'source-paths';writer.write_source_path_list(paths,names)
    for directory in sorted((p for p in root.rglob('*') if p.is_dir()),reverse=True):directory.chmod(0o500)
    root.chmod(0o500)
    other=tmp_path/'other';put(other,b'{}\n')
    snapshot=lambda p:m._read_file(p,'fixture',maximum_bytes=16*1024*1024)
    selection=m.ScalingSourceSelection(m._sealed_directory_snapshot(root,'fixture sealed source'),
        snapshot(other),snapshot(paths),snapshot(other),snapshot(other),snapshot(other))
    loader=m.ScalingParentModules(selection);contract=loader.load('scaling_cli_bootstrap')
    control=tmp_path/'original-control';control.mkdir(mode=0o700)
    contract.stage_dependency_source(DEPENDENCIES,control/'source')
    original=contract.PythonDependencies.provision(control/'source',control/'bundle',control/'inventory.json')
    evidence=tmp_path/'evidence';evidence.mkdir(mode=0o700)
    descriptor=os.open(evidence,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
    operation=object.__new__(m.BootstrapScalingOperation)
    # This fixture does not manufacture a live prepared or process owner. The
    # stage method's only prepared access is guarded by the real dependency owner.
    operation._prepared=types.SimpleNamespace(_dependencies=original,validate=original.verify)
    operation._loader=loader;operation._evidence=evidence;operation._evidence_fd=descriptor
    operation._verifier_python_owner=operation._verifier_python_json=None
    try:
        yield types.SimpleNamespace(m=m,operation=operation,contract=contract,original=original,
            control=control,evidence=evidence,loader=loader)
    finally:
        if operation._verifier_python_owner is not None:operation._verifier_python_owner.close()
        original.close();loader.release();os.close(descriptor)
        for directory in (p for p in tmp_path.rglob('*') if p.is_dir()):directory.chmod(0o700)


def test_actual_retained_package_survives_original_control_removal(staging):
    f=staging;expected=f.contract.dependency_package_census(f.original.paths.source_root)
    original_module=sys.modules.get('blake3')
    f.operation._stage_verifier_python()
    owner=f.operation._verifier_python_owner;binding=json.loads(f.operation._verifier_python_json)
    assert binding==dict(inventory_sha256=owner.paths.inventory_sha256,files=list(expected))
    assert len(expected)==8 and sys.modules.get('blake3') is original_module
    assert hashlib.sha256(owner.paths.inventory.read_bytes()).hexdigest()==binding['inventory_sha256']
    assert f.contract.dependency_package_census(owner.paths.source_root)==expected
    assert f.contract.dependency_package_census(owner.paths.bundle_root)==expected
    f.original.close()
    for directory in (p for p in f.control.rglob('*') if p.is_dir()):directory.chmod(0o700)
    shutil.rmtree(f.control)
    owner.verify();f.loader.verify()


def test_actual_retained_package_tamper_rejects(staging):
    f=staging;f.operation._stage_verifier_python();owner=f.operation._verifier_python_owner
    target=owner.paths.bundle_root/'blake3/__init__.py';target.chmod(0o600)
    target.write_bytes(target.read_bytes()+b'\n# changed retained package\n');target.chmod(0o400)
    with pytest.raises(f.contract.ScalingBootstrapError):owner.verify()


def test_stage_refuses_existing_retained_destination_without_replacing_it(staging):
    f=staging;target=f.evidence/'scaling-verifier-python';target.mkdir(mode=0o700)
    keep=target/'existing';keep.write_bytes(b'keep original')
    with pytest.raises(FileExistsError):f.operation._stage_verifier_python()
    assert keep.read_bytes()==b'keep original' and f.operation._verifier_python_owner is None
    f.original.verify()


def test_stage_checks_original_admitted_package_before_any_destination(staging):
    f=staging;source=f.original.paths.source_root/'blake3/__init__.py';source.chmod(0o600)
    source.write_bytes(source.read_bytes()+b'\n# original changed\n');source.chmod(0o400)
    with pytest.raises(f.contract.ScalingBootstrapError):f.operation._stage_verifier_python()
    assert not (f.evidence/'scaling-verifier-python').exists()
