"""Synthetic fixed-wheel origin controls; no pip or real native execution.

The unchanged maintained wheel harness executes once, then its original ZIP and
RECORD fixture builders are reused. Tiny dependency members and console programs
are inert bytes. The native/SDK join uses real sole-verifier installed owners.
"""
from __future__ import annotations

from dataclasses import replace
import hashlib
import io
import json
from pathlib import Path
import stat
import sys
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_dependency_archive as archive
import sorafs_python_dependency_install as installed
from sorafs_python_dependency_inputs import DependencyWheel, MODULES, DIRECT_VERSIONS
from sorafs_sdk_artifact_index import FileReference
import python_wheel_byte_owner_test as original_tests


@pytest.fixture(scope="module")
def harness(tmp_path_factory):
    fixture_root = tmp_path_factory.mktemp("original-wheel-harness") / "fixtures"
    paths = (original_tests.SHELL_HARNESS, original_tests.VERIFIER)
    before = [path.read_bytes() for path in paths]
    namespace = {"__name__": "__main__", "__file__": str(paths[0])}
    argv = sys.argv
    try:
        sys.argv = [str(paths[0]), str(paths[1]), str(fixture_root)]
        body = original_tests.extract_original_harness(before[0])
        exec(compile(body, str(paths[0]) + ":unchanged-wheel-harness", "exec"), namespace)
    finally:
        sys.argv = argv
    assert [path.read_bytes() for path in paths] == before
    return namespace


def original_wheel(harness, tmp_path, module="pytest", mutate=None):
    version = DIRECT_VERSIONS.get(module, "1.0.0")
    dist = module.replace("-", "_") + "-" + version + ".dist-info"
    member = harness["member"]
    root = archive.ROOTS[module][0]
    entries = [member(root + "/__init__.py", b"# inert original dependency\n"),
               member(dist + "/METADATA", f"Metadata-Version: 2.3\nName: {module}\nVersion: {version}\n".encode()),
               member(dist + "/WHEEL", b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n")]
    if module == "pip":
        entries.append(member(dist + "/entry_points.txt", b"[console_scripts]\npip=pip._internal.cli.main:main\npip3=pip._internal.cli.main:main\n"))
    if module == "pytest":
        entries.append(member("pytest/__init__.py", b"# inert pytest facade\n"))
        entries.append(member(dist + "/entry_points.txt", b"[console_scripts]\npytest=pytest:console_main\npy.test=pytest:console_main\n"))
    if module == "cffi":
        entries.append(member("_cffi_backend.cpython-312-test.so", b"inert native dependency"))
        entries.append(member(dist + "/entry_points.txt", b"[console_scripts]\ncffi-gen-src=cffi._cffi_gen_src:run\n[distutils.setup_keywords]\ncffi_modules=cffi.setuptools_ext:cffi_modules\n"))
    if mutate:
        entries = mutate(entries, dist)
    entries = harness["with_record"](entries, dist + "/RECORD")
    path = tmp_path / (module + ".whl")
    harness["write_wheel"](path, entries)
    raw = path.read_bytes()
    owner = DependencyWheel(module, version, FileReference(str(path), hashlib.sha256(raw).hexdigest(), len(raw)))
    return archive.parse_dependency_wheel(raw, wheel=owner), entries


def record_bytes(harness, bodies, name):
    return harness["with_record"]([harness["member"](path, raw) for path, raw in sorted(bodies.items())], name)[-1][1]


def install_bytes(harness, contents, site, dist, wheel_path, digest, scripts=()):
    bodies = {info.filename: raw for info, raw in contents if not info.is_dir() and info.filename != dist + "/RECORD"}
    bodies[dist + "/INSTALLER"] = b"pip\n"
    bodies[dist + "/REQUESTED"] = b""
    bodies[dist + "/direct_url.json"] = json.dumps({"archive_info": {"hashes": {"sha256": digest}}, "url": wheel_path.as_uri()}).encode()
    for name in scripts:
        bodies["../../../bin/" + name] = b"# retained inert generated console program\n"
    bodies[dist + "/RECORD"] = record_bytes(harness, bodies, dist + "/RECORD")
    for name, raw in bodies.items():
        path = site / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)


def full_environment(harness, tmp_path):
    environment = tmp_path / "environment"
    site = environment / installed.SITE.rstrip("/")
    site.mkdir(parents=True)
    sdk = []
    for owner, contents in ((archive.verifier.NATIVE_OWNER, harness["valid_entries"]()),
                            (archive.verifier.SDK_OWNER, harness["sdk_entries"])):
        path = tmp_path / (owner.package + ".whl")
        harness["write_wheel"](path, contents)
        seal = archive.verifier.seal_wheel(path)
        wheel = archive.verifier.preflight_wheel(path, seal.render(), owner=owner, extension_suffixes=(".abi3.so",))
        install_bytes(harness, contents, site, wheel.dist_info_root, path, seal.sha256)
        layout = archive.verifier.derive_installed_layout(environment_root=environment, site_roots={site}, wheel=wheel)
        sdk.append(archive.verifier.verify_installed_files(wheel, layout).content)
    archives = []
    paths = {}
    for module in MODULES:
        parsed, entries = original_wheel(harness, tmp_path, module)
        archives.append(parsed)
        paths[module] = Path(parsed.wheel.file.path)
        install_bytes(harness, entries, site, parsed.dist_info_root, paths[module], parsed.wheel.file.sha256, parsed.console_scripts)
    files = {path.relative_to(environment).as_posix(): path.read_bytes()
             for path in environment.rglob("*") if path.is_file()}
    kwargs = dict(environment=environment, wheel_paths_by_module=paths, native_sdk_content=tuple(sdk))
    return tuple(archives), files, kwargs


def test_all_fixed_owners_join_actual_original_and_installed_bytes(harness, tmp_path):
    archives, files, kwargs = full_environment(harness, tmp_path)
    result = installed.verify_dependency_install(archives, files, **kwargs)
    assert {row.module for row in result.files} == set(MODULES)
    assert {row.path for row in result.files if row.path.startswith("bin/")} == {
        "bin/pip", "bin/pip3", "bin/pip3.12", "bin/pytest", "bin/py.test", "bin/cffi-gen-src"}
    assert result.files == tuple(sorted(result.files, key=lambda row: row.path))
    assert all((row.sha256, row.size) == (hashlib.sha256(files[row.path]).hexdigest(), len(files[row.path])) for row in result.files)


@pytest.mark.parametrize("mutation", ["foreign-root", "data-root", "startup", "pyc", "symlink", "case", "parent-case", "file-parent", "metadata-name", "metadata-version", "preseed", "entry-group", "entry-traversal", "entry-syntax", "wheel-layout"])
def test_dependency_archive_rejects_structurally_valid_substitutions(harness, tmp_path, mutation):
    original_wheel(harness, tmp_path)
    def change(entries, dist):
        member = harness["member"]
        if mutation == "foreign-root": entries.append(member("iroha_native/foreign.py"))
        elif mutation == "data-root": entries.append(member(dist.removesuffix(".dist-info") + ".data/purelib/evil.py"))
        elif mutation == "startup": entries.append(member("_pytest/SiteCustomize.abi3.so"))
        elif mutation == "pyc": entries.append(member("_pytest/cache.pyc"))
        elif mutation == "symlink": entries.append(member("_pytest/alias.py", b"__init__.py", stat.S_IFLNK | 0o777))
        elif mutation == "case": entries.append(member("_pytest/__INIT__.py"))
        elif mutation == "parent-case": entries.extend((member("_pytest/Sub/a.py"),member("_pytest/sub/b.py")))
        elif mutation == "file-parent": entries.extend((member("_pytest/sub"),member("_pytest/sub/b.py")))
        elif mutation == "metadata-name": entries = [(info, raw.replace(b"Name: pytest", b"Name: pip")) for info,raw in entries]
        elif mutation == "metadata-version": entries = [(info, raw.replace(b"Version: 9.0.3", b"Version: 9.0.2")) for info,raw in entries]
        elif mutation == "preseed": entries.append(member(dist+"/INSTALLER",b"pip\n"))
        elif mutation == "entry-group": entries = [(info,raw+b"[pytest11]\nevil=evil:start\n" if info.filename.endswith("entry_points.txt") else raw) for info,raw in entries]
        elif mutation == "entry-traversal": entries = [(info,raw.replace(b"pytest=pytest:", b"../pytest=pytest:")) for info,raw in entries]
        elif mutation == "entry-syntax": entries = [(info,b"malformed config\n" if info.filename.endswith("entry_points.txt") else raw) for info,raw in entries]
        else: entries = [(info,raw.replace(b"Wheel-Version: 1.0",b"Wheel-Version: 2.0")) for info,raw in entries]
        return entries
    with pytest.raises((ValueError, RuntimeError)):
        original_wheel(harness, tmp_path, mutate=change)


@pytest.mark.parametrize("mutation", ["appended", "prepended", "truncated", "arbitrary", "record", "pin", "owner"])
def test_original_archive_envelope_and_record_primitives_stay_authoritative(harness, tmp_path, mutation):
    parsed, entries = original_wheel(harness, tmp_path)
    raw, owner = parsed.raw, parsed.wheel
    if mutation == "appended": raw += b"foreign"
    elif mutation == "prepended": raw = b"foreign" + raw
    elif mutation == "truncated": raw = raw[:-1]
    elif mutation == "arbitrary": raw = b"not an archive"
    elif mutation == "record":
        entries[-1] = (entries[-1][0], entries[-1][1].replace(b",26\n", b",999\n") + entries[-1][1].splitlines(keepends=True)[0])
        path = tmp_path / "record.whl"; harness["write_wheel"](path, entries);raw = path.read_bytes()
    elif mutation == "owner": owner = replace(owner,module="iroha_native")
    if mutation != "pin": owner = replace(owner,file=replace(owner.file,sha256=hashlib.sha256(raw).hexdigest(),size=len(raw)))
    else: raw = raw[:-1] + bytes((raw[-1]^1,))
    with pytest.raises((ValueError, RuntimeError, zipfile.BadZipFile)):
        archive.parse_dependency_wheel(raw,wheel=owner)


@pytest.mark.parametrize("mutation", ["code", "native", "missing", "extra", "outside-owner", "record-forgery", "record-missing", "record-extra", "direct-url", "installer", "requested", "script-missing", "script-record", "startup", "bytecode", "sdk-code", "sdk-owner", "projection", "module-order", "path-map"])
def test_installed_bytes_refuse_each_independent_wrong_origin(harness,tmp_path,mutation):
    archives,files,kwargs = full_environment(harness,tmp_path)
    installed.verify_dependency_install(archives,files,**kwargs)
    record = installed.SITE + archives[-1].dist_info_root + "/RECORD"
    if mutation == "code": files[installed.SITE+"urllib3/__init__.py"] = b"substitute"
    elif mutation == "native": files[installed.SITE+"_cffi_backend.cpython-312-test.so"] = b"substitute"
    elif mutation == "missing": del files[installed.SITE+"urllib3/__init__.py"]
    elif mutation == "extra": files[installed.SITE+"urllib3/extra.py"] = b"unexpected"
    elif mutation == "outside-owner": files[installed.SITE+"evil.py"] = b"unexpected"
    elif mutation == "record-forgery":
        files[installed.SITE+"urllib3/__init__.py"] = b"substitute"
        selected={name.removeprefix(installed.SITE):raw for name,raw in files.items() if name.startswith((installed.SITE+"urllib3/",installed.SITE+archives[-1].dist_info_root+"/")) and name != record}
        files[record]=record_bytes(harness,selected,record.removeprefix(installed.SITE))
    elif mutation == "record-missing": files[record]=b"\n".join(files[record].splitlines()[1:])+b"\n"
    elif mutation == "record-extra": files[record]+=b"../../../outside,,\n"
    elif mutation == "direct-url": files[installed.SITE+archives[-1].dist_info_root+"/direct_url.json"]=b'{}'
    elif mutation == "installer": files[installed.SITE+archives[-1].dist_info_root+"/INSTALLER"]=b"other\n"
    elif mutation == "requested": del files[installed.SITE+archives[-1].dist_info_root+"/REQUESTED"]
    elif mutation == "script-missing": del files["bin/pytest"]
    elif mutation == "script-record": files["bin/pytest"] += b"changed"
    elif mutation == "startup": files[installed.SITE+"SiteCustomize.abi3.so"]=b"startup"
    elif mutation == "bytecode": files[installed.SITE+"pytest/__pycache__/cache.pyc"]=b"bytecode"
    elif mutation == "sdk-code": files[installed.SITE+"iroha_python/__init__.py"]=b"changed"
    elif mutation == "sdk-owner": kwargs["native_sdk_content"]=(kwargs["native_sdk_content"][0],)*2
    elif mutation == "projection": archives=(replace(archives[0],members=()),*archives[1:])
    elif mutation == "module-order": archives=tuple(reversed(archives))
    else: kwargs["wheel_paths_by_module"]["pytest"] = tmp_path / "foreign.whl"
    with pytest.raises((ValueError, RuntimeError)):
        installed.verify_dependency_install(archives,files,**kwargs)


def test_generated_console_bytes_are_only_retained_observations(harness,tmp_path):
    archives,files,kwargs=full_environment(harness,tmp_path)
    script="bin/pytest";files[script]=b"different pip-generated program, never imported or on PATH\n"
    pytest_archive=next(value for value in archives if value.wheel.module=="pytest")
    record=installed.SITE+pytest_archive.dist_info_root+"/RECORD"
    # Keep the exact original member inventory, replacing only this modeled
    # generated file's retained observation and its installed RECORD hash.
    old=files[record].decode().splitlines()
    fields="../../../"+script+","+harness["record_hash"](files[script])+","+str(len(files[script]))
    files[record] = ("\n".join(fields if row.startswith("../../../"+script+",") else row for row in old)+"\n").encode()
    result=installed.verify_dependency_install(archives,files,**kwargs)
    assert next(row for row in result.files if row.path==script).generated


def test_archive_exact_bounds_and_installed_capacity(harness,tmp_path,monkeypatch):
    parsed,_=original_wheel(harness,tmp_path)
    with zipfile.ZipFile(io.BytesIO(parsed.raw)) as source:
        count=len(source.infolist());total=sum(info.file_size for info in source.infolist())
    monkeypatch.setattr(archive,"MAX_MEMBERS",count)
    monkeypatch.setattr(archive,"MAX_TOTAL_BYTES",total)
    assert archive.parse_dependency_wheel(parsed.raw,wheel=parsed.wheel)==parsed
    monkeypatch.setattr(archive,"MAX_MEMBERS",count-1)
    with pytest.raises(ValueError,match="member bound"):archive.parse_dependency_wheel(parsed.raw,wheel=parsed.wheel)
    monkeypatch.undo()
    archives,files,kwargs=full_environment(harness,tmp_path)
    monkeypatch.setattr(installed,"MAX_ENVIRONMENT_BYTES",sum(map(len,files.values())))
    installed.verify_dependency_install(archives,files,**kwargs)
    monkeypatch.setattr(installed,"MAX_ENVIRONMENT_BYTES",sum(map(len,files.values()))-1)
    with pytest.raises(ValueError,match="byte bound"):installed.verify_dependency_install(archives,files,**kwargs)


@pytest.mark.parametrize("name", ["pytest", "PyTest"])
def test_competing_console_owners_are_rejected_even_with_valid_original_records(harness,tmp_path,name):
    archives,files,kwargs=full_environment(harness,tmp_path)
    installed.verify_dependency_install(archives,files,**kwargs)
    def collide(entries,dist):
        return [(info,raw.replace(b"cffi-gen-src=",name.encode()+b"=") if info.filename.endswith("entry_points.txt") else raw) for info,raw in entries]
    changed,entries=original_wheel(harness,tmp_path,"cffi",mutate=collide)
    archives=tuple(changed if value.wheel.module=="cffi" else value for value in archives)
    site=kwargs["environment"] / installed.SITE.rstrip("/")
    install_bytes(harness,entries,site,changed.dist_info_root,Path(changed.wheel.file.path),changed.wheel.file.sha256,changed.console_scripts)
    files={path.relative_to(kwargs["environment"]).as_posix():path.read_bytes() for path in kwargs["environment"].rglob("*") if path.is_file()}
    with pytest.raises((ValueError,RuntimeError),match="alias|multiple owners"):
        installed.verify_dependency_install(archives,files,**kwargs)


def test_native_sdk_join_requires_actual_sole_verifier_result_types(harness,tmp_path):
    archives,files,kwargs=full_environment(harness,tmp_path)
    from types import SimpleNamespace
    kwargs["native_sdk_content"] = tuple(SimpleNamespace(**vars(value)) for value in kwargs["native_sdk_content"])
    with pytest.raises(ValueError,match="sole verifier"):
        installed.verify_dependency_install(archives,files,**kwargs)
