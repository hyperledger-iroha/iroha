"""Synthetic runtime/bundle/opaque-dependency custody, never Python qualification.

All fixtures are target-local ordinary files and tiny inert byte strings. Tests
exercise real descriptors, links, inventories and streams without executing any
fixture interpreter, installing wheels, fetching dependencies or invoking Cargo.
"""
from __future__ import annotations

import copy
from dataclasses import replace
import hashlib
import io
import json
import os
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_runtime_inputs as contract
import sorafs_python_runtime_custody as custody
import sorafs_python_dependency_inputs as dependencies
import sorafs_sdk_artifact_index as index


def identity(path: Path, raw: bytes | None = None):
    raw = path.read_bytes() if raw is None else raw
    return {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def parsed(value):
    raw = contract.canonical_json(value)
    return contract.parse_runtime_manifest(raw, expected_sha256=hashlib.sha256(raw).hexdigest())


def fixture(tmp_path: Path, *, zip_present=False, excluded="absent", alias=False):
    home = tmp_path / "runtime"
    lib = home / "lib/python3.12"
    files = {"os.py": b"# os\n", "site.py": b"# site\n", "sysconfig.py": b"# sysconfig\n",
             "encodings/__init__.py": b"# encodings\n", "lib-dynload/module.so": b"inert extension"}
    for name, body in files.items():
        path = lib / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(body)
    executable = home / "bin/python3.12"; executable.parent.mkdir(); executable.write_bytes(b"inert executable"); executable.chmod(0o700)
    shared = home / "libpython3.12.so"; shared.write_bytes(b"inert shared runtime")
    zip_path = home / "lib/python312.zip"
    if zip_present: zip_path.write_bytes(b"pinned opaque stdlib zip")
    excluded_target = None
    if excluded == "directory": (lib / "site-packages").mkdir()
    elif excluded == "symlink":
        (home / "packages").mkdir(); excluded_target = "../../packages"
        (lib / "site-packages").symlink_to(excluded_target)
    links = []
    if alias:
        (lib / "config").mkdir(); target = "../../../libpython3.12.so"
        (lib / "config/libpython.so").symlink_to(target)
        links.append({"path": "config/libpython.so", "target": target, "resolved": str(shared)})
    dirs = sorted(str(path.relative_to(lib)) for path in lib.rglob("*")
                  if path.is_dir() and not path.is_symlink() and path.name != "site-packages")
    value = {"schema": contract.SCHEMA, "platform": "darwin", "version": "3.12.14",
             "executable": identity(executable), "shared_runtime": [identity(shared)],
             "stdlib": {"root": str(lib), "files": [{**identity(lib / name), "path": name} for name in sorted(files)],
                        "directories": dirs, "links": links},
             "stdlib_zip": identity(zip_path) if zip_present else {"path": str(zip_path), "sha256": None, "size": None},
             "site_packages": {"kind": excluded, "target": excluded_target}}
    return value


@pytest.mark.parametrize("zip_present,excluded,alias", [(False,"absent",False),(True,"directory",False),(False,"symlink",True)])
def test_actual_original_tree_roundtrip_and_bounded_stream(tmp_path, zip_present, excluded, alias):
    manifest = parsed(fixture(tmp_path, zip_present=zip_present, excluded=excluded, alias=alias))
    owner = custody.OriginalPythonRuntime(manifest)
    class Partial(io.BytesIO):
        def write(self, raw): return super().write(raw[:3])
    with owner:
        owner.recheck(); stream = Partial(); result = owner.write_bundle(stream)
        bundle = contract.parse_runtime_bundle(stream.getvalue(), expected_manifest_sha256=manifest.sha256)
        assert result.sha256 == hashlib.sha256(bundle.raw).hexdigest() and result.size == len(bundle.raw)
        assert bundle.manifest == manifest
        for row in manifest.files(): assert bundle.member_bytes(row.path) == Path(row.path).read_bytes()
        with pytest.raises(ValueError, match="absent"): bundle.member_bytes("/foreign")
    assert not owner._parents
    with pytest.raises(ValueError, match="active"): owner.recheck()
    with pytest.raises(ValueError, match="once"): owner.__enter__()


@pytest.mark.parametrize("mutation", ["schema","platform","version","bool","negative","missing-root","order","duplicate","directory-missing","directory-alias","link-escape","link-directory","zip-slot","zip-null","site-target","startup","extra","noncanonical","pin"])
def test_manifest_rejects_incomplete_ambiguous_and_unpinned_layouts(tmp_path, mutation):
    value = fixture(tmp_path, alias=True)
    if mutation == "schema": value["schema"] += ".old"
    elif mutation == "platform": value["platform"] = "win32"
    elif mutation == "version": value["version"] = "3.13.1"
    elif mutation == "bool": value["executable"]["size"] = True
    elif mutation == "negative": value["executable"]["size"] = -1
    elif mutation == "missing-root": value["stdlib"]["files"] = value["stdlib"]["files"][:-1]
    elif mutation == "order": value["stdlib"]["files"].reverse()
    elif mutation == "duplicate": value["stdlib"]["files"].append(value["stdlib"]["files"][0])
    elif mutation == "directory-missing": value["stdlib"]["directories"].remove("encodings")
    elif mutation == "directory-alias": value["stdlib"]["directories"].append("os.py"); value["stdlib"]["directories"].sort()
    elif mutation == "link-escape": value["stdlib"]["links"][0]["resolved"] = "/foreign/library.so"
    elif mutation == "link-directory": value["stdlib"]["links"][0]["path"] = "encodings"
    elif mutation == "zip-slot": value["stdlib_zip"]["path"] += ".foreign"
    elif mutation == "zip-null": value["stdlib_zip"]["size"] = 0
    elif mutation == "site-target": value["site_packages"]["target"] = "foreign"
    elif mutation == "startup": value["stdlib"]["files"].append({**value["stdlib"]["files"][0], "path":"sitecustomize.py"});value["stdlib"]["files"].sort(key=lambda row:row["path"])
    elif mutation == "extra": value["qualified"] = True
    raw = contract.canonical_json(value)
    if mutation == "noncanonical": raw += b" "
    pin = hashlib.sha256(raw).hexdigest() if mutation != "pin" else "1" * 64
    with pytest.raises(ValueError): contract.parse_runtime_manifest(raw, expected_sha256=pin)


@pytest.mark.parametrize("mutation", ["member","missing","extra","directory","link","hardlink","fifo","absent-zip","dangling-zip","site-kind","mode"])
def test_live_runtime_refuses_actual_filesystem_substitution(tmp_path, mutation):
    value = fixture(tmp_path, alias=True); manifest = parsed(value)
    with custody.OriginalPythonRuntime(manifest) as baseline: baseline.recheck()
    root = Path(manifest.stdlib_root); victim = root / "os.py"
    if mutation == "member": victim.write_bytes(b"# xx\n")
    elif mutation == "missing": victim.unlink()
    elif mutation == "extra": (root / "extra.py").write_bytes(b"extra")
    elif mutation == "directory": (root / "empty-unrecorded").mkdir()
    elif mutation == "link": victim.unlink();victim.symlink_to(root / "site.py")
    elif mutation == "hardlink": os.link(victim, tmp_path / "alias")
    elif mutation == "fifo": victim.unlink();os.mkfifo(victim)
    elif mutation == "absent-zip": Path(manifest.zip_path).write_bytes(b"unexpected")
    elif mutation == "dangling-zip": Path(manifest.zip_path).symlink_to(tmp_path / "absent")
    elif mutation == "site-kind": (root / "site-packages").mkdir()
    elif mutation == "mode": Path(manifest.executable.path).chmod(0o600)
    owner = custody.OriginalPythonRuntime(manifest)
    with pytest.raises((ValueError,OSError,RuntimeError)): owner.__enter__()
    assert not owner._parents


@pytest.mark.parametrize("mutation", ["same-bytes","ancestor","site-target","symlink-target","root-replace"])
def test_original_owner_rechecks_changes_until_context_completion(tmp_path, mutation):
    manifest = parsed(fixture(tmp_path, excluded="symlink", alias=True))
    root = Path(manifest.stdlib_root)
    with pytest.raises((ValueError,OSError,RuntimeError)):
        with custody.OriginalPythonRuntime(manifest):
            if mutation == "same-bytes":
                path = root / "os.py"; path.write_bytes(path.read_bytes())
            elif mutation == "ancestor":
                (root / "encodings").chmod(0o700)
            elif mutation == "site-target":
                (root / "site-packages").unlink();(root / "site-packages").symlink_to("../different")
            elif mutation == "symlink-target":
                link = root / "config/libpython.so";link.unlink();link.symlink_to("../../bin/python3.12")
            else:
                root.rename(root.with_name("original"));root.mkdir()


@pytest.mark.parametrize("mutation", ["magic","length","manifest","body","truncated","trailing"])
def test_bundle_parser_derives_every_byte_and_exact_eof(tmp_path, mutation):
    manifest = parsed(fixture(tmp_path))
    with custody.OriginalPythonRuntime(manifest) as owner:
        output = io.BytesIO();owner.write_bundle(output)
    raw = bytearray(output.getvalue())
    if mutation == "magic": raw[0] ^= 1
    elif mutation == "length": raw[len(contract.MAGIC):len(contract.MAGIC)+8] = b"\xff"*8
    elif mutation == "manifest": raw[len(contract.MAGIC)+9] ^= 1
    elif mutation == "body": raw[-1] ^= 1
    elif mutation == "truncated": del raw[-1]
    else: raw += b"unused"
    with pytest.raises(ValueError): contract.parse_runtime_bundle(bytes(raw),expected_manifest_sha256=manifest.sha256)


def test_output_failure_keeps_original_custody_and_never_invents_a_complete_digest(tmp_path):
    class Refusing(io.BytesIO):
        def write(self, raw): return 0
    manifest = parsed(fixture(tmp_path))
    with custody.OriginalPythonRuntime(manifest) as owner:
        with pytest.raises(ValueError,match="progress"): owner.write_bundle(Refusing())
        owner.recheck()


def test_exact_runtime_and_manifest_bounds(tmp_path, monkeypatch):
    value = fixture(tmp_path); raw = contract.canonical_json(value); pin = hashlib.sha256(raw).hexdigest()
    manifest = contract.parse_runtime_manifest(raw,expected_sha256=pin)
    total = sum(row.size for row in manifest.files())
    monkeypatch.setattr(contract,"MAX_RUNTIME_BYTES",total)
    assert contract.parse_runtime_manifest(raw,expected_sha256=pin) == manifest
    monkeypatch.setattr(contract,"MAX_RUNTIME_BYTES",total-1)
    with pytest.raises(ValueError,match="aggregate"): contract.parse_runtime_manifest(raw,expected_sha256=pin)
    monkeypatch.setattr(contract,"MAX_MANIFEST_BYTES",len(raw)-1)
    with pytest.raises(ValueError,match="bound"): contract.parse_runtime_manifest(raw,expected_sha256=pin)


def dependency_value(tmp_path):
    rows=[]
    for module in dependencies.MODULES:
        path=tmp_path/(module+'.whl');path.write_bytes(('opaque pinned fixture '+module).encode())
        rows.append({'module':module,'version':dependencies.DIRECT_VERSIONS.get(module,'1.0.0'),'file':identity(path)})
    return {'schema':dependencies.SCHEMA,'wheels':rows}


def dep_parse(value):
    raw=contract.canonical_json(value)
    return dependencies.parse_dependency_manifest(raw,expected_sha256=hashlib.sha256(raw).hexdigest())


def test_fixed_dependency_pins_are_source_coupled_and_opaque(tmp_path):
    manifest=dep_parse(dependency_value(tmp_path))
    assert tuple(w.module for w in manifest.wheels)==dependencies.MODULES
    requirements=(ROOT/'scripts/requirements.txt').read_text()
    for name,version in dependencies.DIRECT_VERSIONS.items(): assert name+'=='+version in requirements
    assert b'pytest==8.4.2' in (ROOT/'python/iroha_python/requirements-ci.lock').read_bytes()
    assert not hasattr(manifest,'installed') and not hasattr(manifest,'qualified')


@pytest.mark.parametrize('mutation',['missing','duplicate','order','unknown','old-pytest','range','bool','path-alias','extra','pin'])
def test_dependency_manifest_refuses_weaker_or_incomplete_pins(tmp_path,mutation):
    value=dependency_value(tmp_path)
    if mutation=='missing':value['wheels'].pop()
    elif mutation=='duplicate':value['wheels'][1]=value['wheels'][0]
    elif mutation=='order':value['wheels'].reverse()
    elif mutation=='unknown':value['wheels'][0]['module']='unreviewed'
    elif mutation=='old-pytest':next(w for w in value['wheels'] if w['module']=='pytest')['version']='8.4.2'
    elif mutation=='range':value['wheels'][0]['version']='>=1'
    elif mutation=='bool':value['wheels'][0]['file']['size']=True
    elif mutation=='path-alias':value['wheels'][1]['file']=value['wheels'][0]['file']
    elif mutation=='extra':value['wheels'][0]['accepted']=True
    raw=contract.canonical_json(value)
    with pytest.raises(ValueError):dependencies.parse_dependency_manifest(raw,expected_sha256='1'*64 if mutation=='pin' else hashlib.sha256(raw).hexdigest())


def index_fixture(tmp_path, manifest):
    # Genuine complete index parser/descriptor owner, with explicitly synthetic
    # unrelated consumer artifacts; no fake candidate or execution token enters APIs.
    bodies={};consumers=[]
    mapping={wheel.module:'inputs/'+wheel.module+'.whl' for wheel in manifest.wheels}
    for wheel in manifest.wheels:bodies[mapping[wheel.module]]=Path(wheel.file.path).read_bytes()
    for name,suffix in zip(index.CONSUMERS,index.SUFFIXES,strict=True):
        artifact=name+suffix;execution=artifact if name=='java_source_kotlin' else name+'-execution.zip'
        bodies[artifact]=('artifact '+name).encode();bodies[execution]=('execution '+name).encode()
        inputs=sorted(mapping.values()) if name=='python' else [name+'-input.bin']
        if name!='python':bodies[inputs[0]]=name.encode()
        if name=='java_source_kotlin':inputs.append('kotlin_jvm.zip');inputs.sort()
        consumers.append({'consumer':name,'version':'1.0.0','artifact':artifact,'execution':execution,'inputs':inputs})
    candidate={'source_commit':'a'*40,'workspace_source_manifest_sha256':'b'*64}
    value={'schema':index.SCHEMA,'candidate':candidate,'files':{name:{'sha256':hashlib.sha256(raw).hexdigest(),'size':len(raw)} for name,raw in sorted(bodies.items())},'consumers':consumers}
    parsed_index=index.parse_index(contract.canonical_json(value),expected_source_commit=candidate['source_commit'],expected_source_manifest_sha256=candidate['workspace_source_manifest_sha256'])
    root=tmp_path/'original-index';root.mkdir()
    for name,raw in bodies.items():
        path=root/name;path.parent.mkdir(exist_ok=True);path.write_bytes(raw)
    return index.OpenedIndexFiles(root,parsed_index),mapping


@pytest.mark.parametrize('mutation',[None,'wrong-input','duplicate-map','substitution','inactive'])
def test_dependency_index_join_consumes_actual_original_descriptors(tmp_path,mutation):
    manifest=dep_parse(dependency_value(tmp_path));owner,mapping=index_fixture(tmp_path,manifest)
    if mutation=='inactive':
        with pytest.raises(ValueError):dependencies.authenticate_dependency_inputs(owner,manifest,paths_by_module=mapping)
        return
    with owner:
        assert dependencies.authenticate_dependency_inputs(owner,manifest,paths_by_module=mapping)==manifest.wheels
        if mutation is None:return
        if mutation=='wrong-input':mapping['pip']='javascript-input.bin'
        elif mutation=='duplicate-map':mapping['pip']=mapping['pytest']
        else:
            changed=list(manifest.wheels);changed[0]=dependencies.DependencyWheel(changed[0].module,changed[0].version,index.FileReference(changed[0].file.path,'1'*64,changed[0].file.size))
            manifest=dependencies.DependencyManifest(manifest.raw,manifest.sha256,tuple(changed))
        with pytest.raises(ValueError):dependencies.authenticate_dependency_inputs(owner,manifest,paths_by_module=mapping)


@pytest.mark.parametrize("name", ["sitecustomize.pyc", "USERCUSTOMIZE.PYC", "sitecustomize.cpython-312-darwin.so", "usercustomize.abi3.so", "SiteCustomize"])
def test_all_direct_startup_module_families_are_closed(tmp_path, name):
    value = fixture(tmp_path)
    parsed(value)
    value["stdlib"]["files"].append({**value["stdlib"]["files"][0], "path": name})
    value["stdlib"]["files"].sort(key=lambda row: row["path"])
    with pytest.raises(ValueError, match="startup"):
        parsed(value)


@pytest.mark.parametrize("target", ["../../../bin/python3.12", "missing/../libpython.so", "../os.py/../os.py", "libpython.so", "../../../unowned.so"])
def test_bundle_inventory_itself_derives_symlink_resolution(tmp_path, target):
    value = fixture(tmp_path, alias=True)
    parsed(value)
    value["stdlib"]["links"][0]["target"] = target
    with pytest.raises(ValueError, match="symlink"):
        parsed(value)


def test_declared_link_chain_resolves_through_exact_pinned_links(tmp_path):
    value = fixture(tmp_path, alias=True)
    value["stdlib"]["links"].append({"path": "config/second.so", "target": "libpython.so", "resolved": value["shared_runtime"][0]["path"]})
    manifest = parsed(value)
    assert len(manifest.stdlib_links) == 2


def test_runtime_owner_refuses_a_forged_immutable_projection(tmp_path):
    manifest = parsed(fixture(tmp_path))
    with pytest.raises(ValueError, match="projection"):
        custody.OriginalPythonRuntime(replace(manifest, version="3.12.99"))
