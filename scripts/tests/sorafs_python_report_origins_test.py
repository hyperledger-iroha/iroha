"""Captured origin joins only; synthetic reports never qualify 77 native cases.

The existing wheel harness owns inert archive/RECORD fixtures. This module uses
the real wheel/content/report parsers but executes no fixture interpreter, pip,
SDK package, or native library. Reported physical installed seals are synthetic
observations explicitly lacking live-file authority.
"""
from __future__ import annotations

from dataclasses import asdict, replace
import hashlib
import importlib.machinery
import json
import os
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_report_origins as origins
import sorafs_python_consumer_artifact as artifact
import sorafs_python_runtime_inputs_test as runtime_fixture
import sorafs_python_consumer_artifact_test as report_fixture
from sorafs_python_dependency_install_test import harness, record_bytes
from sorafs_python_dependency_install import SITE

verifier = artifact._VERIFIER


def identity(path, raw):
    return {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


@pytest.fixture
def joined(harness, tmp_path):
    environment = Path("/captured/absent/environment")
    snapshot = Path("/captured/absent/snapshot")
    runtime = runtime_fixture.parsed(runtime_fixture.fixture(tmp_path))
    executable = Path(runtime.executable.path).read_bytes()
    files = {"bin/python3.12": executable, SITE + "pytest/__init__.py": b"# inert pytest fixture\n"}
    sources = {name: b"# inert captured source\n" for name in artifact.FIXED_SOURCES}
    sources[report_fixture.TEST_PATH] = report_fixture.TEST_SOURCE
    sources["python/norito_py/src/norito/__init__.py"] = b"# inert norito fixture\n"
    sources["python/iroha_torii_client/__init__.py"] = b"# inert torii fixture\n"
    wheels, paths, seals, rows = [], [], [], []
    inode = 1
    for owner in (verifier.NATIVE_OWNER, verifier.SDK_OWNER):
        entries = harness["valid_entries"]() if owner.native else harness["sdk_entries"]
        if not owner.native:
            entries = harness["with_record"](
                [(info, raw) for info, raw in entries if not info.filename.endswith("/RECORD")]
                + [harness["member"]("iroha_python/sorafs.py", b"# inert SoraFS consumer fixture\n")],
                "iroha_python-0.0.0.dist-info/RECORD")
        path = harness["write_wheel"](tmp_path / (owner.package + ".whl"), entries)
        raw = path.read_bytes()
        seal = verifier.seal_wheel(path)
        wheel = verifier.parse_wheel_bytes(raw, owner=owner, extension_suffixes=(".abi3.so",))
        record = wheel.dist_info_root + "/RECORD"
        captured = {info.filename: body for info, body in entries if not info.is_dir() and info.filename != record}
        captured[wheel.dist_info_root + "/INSTALLER"] = b"pip\n"
        captured[wheel.dist_info_root + "/REQUESTED"] = b""
        captured[wheel.dist_info_root + "/direct_url.json"] = json.dumps(
            {"url": path.as_uri(), "archive_info": {"hashes": {"sha256": seal.sha256}}}).encode()
        captured[record] = record_bytes(harness, captured, record)
        content = verifier.verify_installed_wheel_bytes(
            wheel, source_uri=path.as_uri(), wheel_sha256=seal.sha256, installed_files=captured)
        files.update({SITE + name: body for name, body in captured.items()})
        installed = []
        for member in content.files:
            observed = verifier.FileSeal(member.sha256, 0, inode, member.size, 1, 1, 0o644)
            installed.append({"path": str(environment / SITE / member.name), "seal": observed.render()})
            inode += 1
        required = ((owner.package, wheel.package_member),
                    ("iroha_native._crypto", wheel.native_member) if owner.native
                    else ("iroha_python.sorafs", "iroha_python/sorafs.py"))
        modules = [{**identity(environment / SITE / member, captured[member]), "member": member,
                    "name": name, "loader": "ExtensionFileLoader" if name.endswith("._crypto") else "SourceFileLoader"}
                   for name, member in required]
        rows.append({"owner": owner.package, "path": str(path), "seal": seal.render(),
                     "version": wheel.metadata_version, "installed_files": installed,
                     "loaded_modules": sorted(modules, key=lambda row: row["name"])})
        wheels.append((wheel, content)); paths.append(path); seals.append(seal)
    value = report_fixture.report.__wrapped__()
    value.update(source_files=[identity(name, raw) for name, raw in sorted(sources.items())], wheels=rows,
                 python={**identity(environment / "bin/python3.12", executable), "version": runtime.version},
                 pytest={**identity(environment / SITE / "pytest/__init__.py", files[SITE + "pytest/__init__.py"]),
                         "version": origins.PYTEST_VERSION})
    value["dependencies"] = []
    for owner, prefix, member in (("norito", "python/norito_py/src/", "norito/__init__.py"),
                                   ("iroha_torii_client", "python/iroha_torii_client/", "__init__.py")):
        root = snapshot / prefix
        value["dependencies"].append({"module": owner, "root": str(root), "loaded_modules": [
            {**identity(root / member, sources[prefix + member]), "name": owner, "loader": "SourceFileLoader"}]})
    report = artifact.parse_report(artifact.canonical_json(value), expected_input_sha256=report_fixture.INPUT,
                                   test_source=report_fixture.TEST_SOURCE)
    kwargs = dict(environment=environment, snapshot=snapshot, sources=sources, environment_files=files,
                  wheels=tuple(wheels), wheel_paths=tuple(paths), wheel_seals=tuple(seals), runtime=runtime)
    return report, kwargs


def check(joined):
    report, kwargs = joined
    return origins.verify_report_origins(report, **kwargs)


def changed_wheel(report, index=0, **fields):
    wheels = list(report.wheels); wheels[index] = replace(wheels[index], **fields)
    return replace(report, wheels=tuple(wheels))


def test_exact_metadata_joins_retain_original_bytes_without_live_authority(joined):
    report, kwargs = joined
    result = check(joined)
    expected = {"installed-metadata/" + wheel.dist_info_root + "/" + name:
                kwargs["environment_files"][SITE + wheel.dist_info_root + "/" + name]
                for wheel, _ in kwargs["wheels"] for name in ("RECORD", "direct_url.json")}
    assert result == expected and len(result) == 4
    assert all(result[name] is raw for name, raw in expected.items())
    result.clear()
    assert check(joined) == expected
    # These are encoded synthetic observations, not executed tests.
    assert len(report.cases) == 77
    assert not any(hasattr(content, "seal") for _, content in kwargs["wheels"])


def test_logical_replay_performs_no_historical_filesystem_or_native_loading(joined, monkeypatch):
    def forbidden(*_args, **_kwargs):
        raise AssertionError("captured report replay attempted filesystem/native authority")
    with monkeypatch.context() as guarded:
        for name in ("open", "resolve", "stat", "lstat", "exists", "read_bytes"):
            guarded.setattr(Path, name, forbidden)
        guarded.setattr(os, "open", forbidden)
        guarded.setattr(os, "stat", forbidden)
        guarded.setattr(importlib.machinery.ExtensionFileLoader, "create_module", forbidden)
        guarded.setattr(importlib.machinery.ExtensionFileLoader, "exec_module", forbidden)
        result = check(joined)
    assert len(result) == 4


@pytest.mark.parametrize("mutation", (
    "python_path", "python_digest", "python_size", "python_version", "runtime_version",
    "captured_python", "missing_python", "pytest_path", "pytest_digest", "pytest_size",
    "pytest_version", "captured_pytest", "source_extra", "source_missing", "source_changed",
    "source_mutable", "reported_source_missing", "reported_source_duplicate",
    "snapshot_root", "environment_root",
))
def test_tools_and_exact_source_inventory_must_join(joined, mutation):
    report, kwargs = joined
    if mutation.startswith("python_"):
        key = {"python_path": "path", "python_digest": "sha256", "python_size": "size", "python_version": "version"}[mutation]
        value = {"path": "/foreign/python3.12", "sha256": "1" * 64, "size": 0, "version": "3.12.13"}[key]
        report = replace(report, python=replace(report.python, **{key: value}))
    elif mutation.startswith("pytest_"):
        key = {"pytest_path": "path", "pytest_digest": "sha256", "pytest_size": "size", "pytest_version": "version"}[mutation]
        value = {"path": "/foreign/pytest/__init__.py", "sha256": "1" * 64, "size": 0, "version": "9.0.2"}[key]
        report = replace(report, pytest=replace(report.pytest, **{key: value}))
    elif mutation == "runtime_version": kwargs["runtime"] = replace(kwargs["runtime"], version="3.12.13")
    elif mutation == "captured_python": kwargs["environment_files"]["bin/python3.12"] += b"changed"
    elif mutation == "missing_python": del kwargs["environment_files"]["bin/python3.12"]
    elif mutation == "captured_pytest": kwargs["environment_files"][SITE + "pytest/__init__.py"] += b"changed"
    elif mutation == "source_extra": kwargs["sources"]["python/norito_py/src/norito/extra.py"] = b"extra"
    elif mutation == "source_missing": del kwargs["sources"]["python/norito_py/src/norito/__init__.py"]
    elif mutation == "source_changed": kwargs["sources"]["python/norito_py/src/norito/__init__.py"] += b"changed"
    elif mutation == "source_mutable": kwargs["sources"]["python/norito_py/src/norito/__init__.py"] = bytearray(b"mutable")
    elif mutation == "reported_source_missing": report = replace(report, source_files=report.source_files[1:])
    elif mutation == "reported_source_duplicate": report = replace(report, source_files=report.source_files + report.source_files[:1])
    elif mutation == "snapshot_root": kwargs["snapshot"] = Path("/other/snapshot")
    else: kwargs["environment"] = Path("/other/environment")
    with pytest.raises(artifact.ArtifactError): origins.verify_report_origins(report, **kwargs)


@pytest.mark.parametrize("mutation", (
    "wheel_owner", "wheel_version", "wheel_path", "wheel_digest", "wheel_inode", "wheel_size",
    "wheel_order", "path_order", "seal_order", "content_pair", "content_source", "content_missing",
    "installed_missing", "installed_order", "installed_digest", "installed_size", "captured_member",
    "captured_record", "captured_direct_url", "captured_extra", "captured_competing_dist", "captured_case",
    "loaded_member", "loaded_path", "loaded_name",
    "loaded_loader", "loaded_digest", "loaded_required_missing",
))
def test_every_wheel_installed_and_loaded_relationship_is_joined(joined, mutation):
    report, kwargs = joined
    wheel = report.wheels[0]
    if mutation in ("wheel_owner", "wheel_version", "wheel_path"):
        key = mutation.removeprefix("wheel_")
        report = changed_wheel(report, **{key: {"owner": "foreign", "version": "0.0.1", "path": "/foreign/native.whl"}[key]})
    elif mutation in ("wheel_digest", "wheel_inode", "wheel_size"):
        key = {"wheel_digest": "sha256", "wheel_inode": "inode", "wheel_size": "size"}[mutation]
        report = changed_wheel(report, seal=replace(wheel.seal, **{key: "1" * 64 if key == "sha256" else getattr(wheel.seal, key) + 1}))
    elif mutation == "wheel_order": report = replace(report, wheels=tuple(reversed(report.wheels)))
    elif mutation == "path_order": kwargs["wheel_paths"] = tuple(reversed(kwargs["wheel_paths"]))
    elif mutation == "seal_order": kwargs["wheel_seals"] = tuple(reversed(kwargs["wheel_seals"]))
    elif mutation.startswith("content_"):
        parsed, content = kwargs["wheels"][0]
        if mutation == "content_pair": content = kwargs["wheels"][1][1]
        elif mutation == "content_source": content = replace(content, source_uri="file:///foreign/native.whl")
        else: content = replace(content, files=content.files[1:])
        kwargs["wheels"] = ((parsed, content), kwargs["wheels"][1])
    elif mutation in ("installed_missing", "installed_order", "installed_digest", "installed_size"):
        files = wheel.installed_files
        if mutation == "installed_missing": files = files[1:]
        elif mutation == "installed_order": files = tuple(reversed(files))
        else:
            key = "sha256" if mutation == "installed_digest" else "size"
            files = (replace(files[0], seal=replace(files[0].seal, **{key: "1" * 64 if key == "sha256" else files[0].seal.size + 1})), *files[1:])
        report = changed_wheel(report, installed_files=files)
    elif mutation.startswith("captured_"):
        parsed = kwargs["wheels"][0][0]
        if mutation in ("captured_extra", "captured_competing_dist", "captured_case"):
            name = {"captured_extra": "iroha_native/extra.py", "captured_competing_dist": "iroha_native-0.1.0.dist-info/METADATA",
                    "captured_case": "IROHA_NATIVE/extra.py"}[mutation]
            kwargs["environment_files"][SITE + name] = b"unowned"
        else:
            name = {"captured_member": parsed.package_member, "captured_record": parsed.dist_info_root + "/RECORD",
                    "captured_direct_url": parsed.dist_info_root + "/direct_url.json"}[mutation]
            kwargs["environment_files"][SITE + name] += b"changed"
    else:
        modules = list(wheel.loaded_modules)
        if mutation == "loaded_required_missing": modules.pop()
        else:
            key = mutation.removeprefix("loaded_")
            key = "sha256" if key == "digest" else key
            values = {"member": kwargs["wheels"][0][0].dist_info_root + "/METADATA", "path": "/foreign/module.py",
                      "name": "iroha_native.foreign", "loader": "ExtensionFileLoader", "sha256": "1" * 64}
            modules[0] = replace(modules[0], **{key: values[key]})
        report = changed_wheel(report, loaded_modules=tuple(modules))
    with pytest.raises(artifact.ArtifactError): origins.verify_report_origins(report, **kwargs)


@pytest.mark.parametrize("mutation", ("root", "module", "path", "digest", "size", "loader", "member", "missing"))
def test_dependency_observations_join_the_supplied_snapshot(joined, mutation):
    report, kwargs = joined
    dependency = report.dependencies[0]
    if mutation == "root": dependency = replace(dependency, root="/foreign/python/norito_py/src")
    elif mutation == "module": dependency = replace(dependency, module="foreign")
    elif mutation == "missing": dependency = replace(dependency, loaded_modules=())
    else:
        key = "sha256" if mutation == "digest" else mutation
        values = {"path": "/foreign/norito/__init__.py", "sha256": "1" * 64, "size": 0,
                  "loader": "ExtensionFileLoader", "member": "norito/__init__.py"}
        module = replace(dependency.loaded_modules[0], **{key: values[key]})
        dependency = replace(dependency, loaded_modules=(module,))
    report = replace(report, dependencies=(dependency, report.dependencies[1]))
    with pytest.raises(artifact.ArtifactError): origins.verify_report_origins(report, **kwargs)


def test_installed_physical_metadata_remains_unverifiable_observation(joined):
    report, kwargs = joined
    expected = check(joined)
    item = report.wheels[0].installed_files[0]
    changed = replace(item, seal=replace(item.seal, device=17, inode=987654321, mtime_ns=99, ctime_ns=100, mode=0o600))
    report = changed_wheel(report, installed_files=(changed, *report.wheels[0].installed_files[1:]))
    assert origins.verify_report_origins(report, **kwargs) == expected
