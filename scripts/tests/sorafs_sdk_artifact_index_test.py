"""Concrete indexed-byte and Java-adapter controls, never release qualification.

The positive Java input uses the explicitly mocked producer from its component
suite. No native library or independent signer is substituted for production.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import sorafs_sdk_artifact_index as index_owner
import sorafs_sdk_java_artifact_verifier as java
import sorafs_java_consumer_artifact as contract
import build_sorafs_java_consumer_artifact as producer
from sorafs_java_consumer_artifact_test import (
    synthetic_producer_inputs, native_manifest_bytes, zip_bytes,
)


def index_value(files=None):
    """Build a complete graph with distinct synthetic non-qualification products."""
    files = {} if files is None else files
    rows = []
    for name, suffix in zip(index_owner.CONSUMERS, index_owner.SUFFIXES, strict=True):
        artifact = name + suffix
        execution = artifact if name == "java_source_kotlin" else name + "-execution.zip"
        extra = name + "-input.bin"
        files.setdefault(artifact, name.encode())
        files.setdefault(execution, (name + " observations").encode())
        files.setdefault(extra, (name + " input").encode())
        inputs = [extra]
        if name == "java_source_kotlin": inputs.append("kotlin_jvm.zip")
        rows.append({"consumer": name, "version": "1.0.0", "artifact": artifact,
                     "execution": execution, "inputs": sorted(inputs)})
    return {"schema": index_owner.SCHEMA,
            "candidate": {"source_commit": "a" * 40, "workspace_source_manifest_sha256": "b" * 64},
            "files": {name: contract.identity(raw) for name, raw in sorted(files.items())},
            "consumers": rows}, files


def parse(value):
    """Use the actual closed parser with an independently supplied candidate."""
    return index_owner.parse_index(contract.canonical_json(value), expected_source_commit="a" * 40,
                                   expected_source_manifest_sha256="b" * 64)


def write_files(root, files):
    """Create exact component fixture inputs inside pytest's owned directory."""
    root.mkdir(parents=True, exist_ok=True)
    for name, raw in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)


def test_index_opens_actual_original_bytes_without_an_execution_claim(tmp_path):
    value, files = index_value(); index = parse(value)
    write_files(tmp_path, files)
    owner = index_owner.OpenedIndexFiles(tmp_path, index)
    with owner as opened:
        for name, raw in files.items(): assert opened.read(name, len(raw)) == raw
        assert index.consumer("java_source_kotlin").inputs == ("java_source_kotlin-input.bin", "kotlin_jvm.zip")
        with pytest.raises(index_owner.IndexError): index.consumer("java_android")
        with pytest.raises(index_owner.IndexError): index.file("absent")
    with pytest.raises(index_owner.IndexError, match="active"): owner.read(next(iter(files)), 1024)
    with pytest.raises(index_owner.IndexError, match="only once"): owner.__enter__()
    assert not hasattr(index, "qualified") and not hasattr(index, "passed")


@pytest.mark.parametrize("mutation", ["schema", "candidate", "manifest", "consumer", "order", "missing", "duplicate",
                                     "version", "bool_size", "zero_size", "digest", "extra", "unconsumed", "path",
                                     "inputs", "format", "execution", "same_artifact", "kotlin_version", "kotlin_join"])
def test_index_rejects_retired_ambiguous_or_unbound_contracts(mutation):
    value, _ = index_value(); row = value["consumers"][3]; first = next(iter(value["files"]))
    if mutation == "schema": value["schema"] += ".old"
    elif mutation == "candidate": value["candidate"]["source_commit"] = "c" * 40
    elif mutation == "manifest": value["candidate"]["workspace_source_manifest_sha256"] = "c" * 64
    elif mutation == "consumer": row["consumer"] = "java_android"
    elif mutation == "order": value["consumers"].reverse()
    elif mutation == "missing": value["consumers"].pop()
    elif mutation == "duplicate": value["consumers"][4] = row
    elif mutation == "version": row["version"] = "01.0.0"
    elif mutation == "bool_size": value["files"][first]["size"] = True
    elif mutation == "zero_size": value["files"][first]["size"] = 0
    elif mutation == "digest": value["files"][first]["sha256"] = "0" * 64
    elif mutation == "extra": row["passed"] = True
    elif mutation == "unconsumed": value["files"]["unconsumed.bin"] = contract.identity(b"unused")
    elif mutation == "path": row["artifact"] = "../escape.zip"
    elif mutation == "inputs": row["inputs"] = ["absent"]
    elif mutation == "format": row["artifact"] = "csharp.nupkg"
    elif mutation == "execution": row["execution"] = "kotlin_jvm-execution.zip"
    elif mutation == "same_artifact": value["files"][row["artifact"]] = value["files"]["kotlin_jvm.zip"]
    elif mutation == "kotlin_version": row["version"] = "2.0.0"
    elif mutation == "kotlin_join": row["inputs"].remove("kotlin_jvm.zip")
    with pytest.raises(ValueError): parse(value)


def test_index_bounds_and_canonical_encoding_precede_file_access(monkeypatch):
    value, _ = index_value(); raw = contract.canonical_json(value)
    with pytest.raises(ValueError, match="canonical"):
        index_owner.parse_index(json.dumps(value).encode(), expected_source_commit="a" * 40, expected_source_manifest_sha256="b" * 64)
    for field, limit in (("MAX_INDEX_BYTES", len(raw) - 1), ("MAX_FILES", 1), ("MAX_FILE_BYTES", 1), ("MAX_TOTAL_BYTES", 1)):
        with monkeypatch.context() as context:
            context.setattr(index_owner, field, limit)
            with pytest.raises(ValueError): parse(value)


@pytest.mark.parametrize("mutation", ["bytes", "size", "symlink", "hardlink", "directory", "parent_symlink"])
def test_index_file_custody_rejects_substitution_before_open(tmp_path, mutation):
    value, files = index_value(); index = parse(value); write_files(tmp_path, files)
    name = next(iter(files)); path = tmp_path / name
    if mutation == "bytes": path.write_bytes(b"X" * len(files[name]))
    elif mutation == "size": path.write_bytes(files[name] + b"X")
    elif mutation == "symlink": path.unlink(); path.symlink_to(tmp_path / next(n for n in files if n != name))
    elif mutation == "hardlink": os.link(path, tmp_path / "extra-link")
    elif mutation == "directory": path.unlink(); path.mkdir()
    elif mutation == "parent_symlink":
        alias = tmp_path / "alias"; alias.symlink_to(tmp_path, target_is_directory=True); tmp_path = alias
    with pytest.raises((ValueError, OSError, RuntimeError)):
        with index_owner.OpenedIndexFiles(tmp_path, index): pass


def test_index_fifo_leaf_refuses_without_waiting_for_a_writer(tmp_path):
    value, files = index_value(); write_files(tmp_path, files)
    path = tmp_path / next(iter(files)); path.unlink(); os.mkfifo(path)
    source = tmp_path / "index.json"; source.write_bytes(contract.canonical_json(value))
    program = """import sys
from pathlib import Path
import sorafs_sdk_artifact_index as owner
root = Path(sys.argv[1])
index = owner.parse_index((root/'index.json').read_bytes(), expected_source_commit='a'*40, expected_source_manifest_sha256='b'*64)
try:
    with owner.OpenedIndexFiles(root, index): pass
except owner.IndexError:
    sys.exit(0)
sys.exit(2)
"""
    result = subprocess.run([sys.executable, "-c", program, str(tmp_path)],
                            env={**os.environ, "PYTHONPATH": str(ROOT / "scripts")},
                            capture_output=True, timeout=5)
    assert result.returncode == 0, result.stderr.decode()


@pytest.mark.parametrize("mutation", ["contents", "replaced", "ancestor", "read_bound"])
def test_index_retains_original_owner_through_completion(tmp_path, mutation):
    root = tmp_path / "inputs"; value, files = index_value(); write_files(root, files); index = parse(value)
    name = next(iter(files)); path = root / name
    with pytest.raises(ValueError):
        with index_owner.OpenedIndexFiles(root, index) as opened:
            assert opened.read(name, 1024) == files[name]
            if mutation == "contents": path.write_bytes(b"X" * len(files[name]))
            elif mutation == "replaced":
                replacement = root / "replacement"; replacement.write_bytes(files[name]); replacement.replace(path)
            elif mutation == "ancestor":
                root.rename(tmp_path / "original"); write_files(root, files)
            elif mutation == "read_bound": opened.read(name, len(files[name]) - 1)


def java_fixture(tmp_path, monkeypatch):
    """Retain all actual synthetic inputs; only process/native calls are mocked."""
    parser = producer.parse_native_manifest
    args, _, _ = synthetic_producer_inputs(tmp_path, monkeypatch)
    args.native_manifest.write_bytes(native_manifest_bytes())
    monkeypatch.setattr(producer, "parse_native_manifest", parser)
    for path in args.jdk_home.rglob("*"):
        if path.is_file() and path.name != "release": path.write_bytes(str(path.relative_to(args.jdk_home)).encode())
    for name in java.TOOLS:
        target = args.source_root / "scripts" / name
        target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes((ROOT / "scripts" / name).read_bytes())
    result = producer.produce(args)
    artifact = Path(result["artifact"]).read_bytes()
    distribution = zip_bytes([("iroha-mobile-sdk-android-1.0.0/core-jvm/core.jar", args.core_jar.read_bytes()),
                              ("iroha-mobile-sdk-android-1.0.0/client-android/client-android-release.aar", args.client_aar.read_bytes())])
    value, files = index_value({"java_source_kotlin.zip": artifact, "kotlin_jvm.zip": distribution})
    del files["java_source_kotlin-input.bin"]
    originals = {"core.jar": args.core_jar, "client.aar": args.client_aar,
                 "native.bin": args.native_artifact, "native.json": args.native_manifest,
                 "dependencies.json": args.dependency_manifest}
    for row in json.loads(args.dependency_manifest.read_bytes())["jars"]:
        originals["dependencies/" + Path(row["path"]).name] = Path(row["path"])
    for path in args.jdk_home.rglob("*"):
        if path.is_file(): originals["jdk/" + path.relative_to(args.jdk_home).as_posix()] = path
    files.update({"java-inputs/" + name: path.read_bytes() for name, path in originals.items()})
    value["consumers"][3]["inputs"] = sorted(["kotlin_jvm.zip", *("java-inputs/" + name for name in originals)])
    value["files"] = {name: contract.identity(raw) for name, raw in sorted(files.items())}
    return args, value, files


def verify_fixture(args, value, files, root):
    """Invoke the real adapter over real opened fixture files, with no verifier mock."""
    value["files"] = {name: contract.identity(raw) for name, raw in sorted(files.items())}
    write_files(root, files); index = parse(value)
    with index_owner.OpenedIndexFiles(root, index) as opened:
        return java.verify_java_consumer(index, opened, trusted_source_root=args.source_root)


def test_java_adapter_rederives_two_lane_observations_from_original_bytes(tmp_path, monkeypatch):
    args, value, files = java_fixture(tmp_path, monkeypatch)
    result = verify_fixture(args, value, files, tmp_path / "indexed")
    assert result.cases == (("jvm", contract.GROUPS), ("android-host", contract.GROUPS))
    assert result.kotlin_artifact_sha256 == contract.identity(files["kotlin_jvm.zip"])["sha256"]
    assert not hasattr(result, "qualified") and not hasattr(result, "passed")


@pytest.mark.parametrize("mutation", ["kotlin_package", "native", "native_manifest", "dependency", "jdk", "source",
                                     "fixture", "archive_extra", "report", "full_stream", "cases", "loaded",
                                     "compiled", "missing_probe", "split_root", "retained", "unknown_manifest",
                                     "runner_origin", "runner_missing", "probe_origin", "dependency_projection",
                                     "missing_dependency_projection", "dependency_origin"])
def test_java_adapter_rejects_resealed_substitutions(tmp_path, monkeypatch, mutation):
    args, value, files = java_fixture(tmp_path, monkeypatch)
    members = contract.archive_members(files["java_source_kotlin.zip"])
    manifest = json.loads(members["manifest.json"])
    if mutation == "kotlin_package":
        archive = contract.archive_members(files["kotlin_jvm.zip"])
        archive[next(name for name in archive if name.endswith("core.jar"))] += b"replacement"
        files["kotlin_jvm.zip"] = zip_bytes(list(archive.items()))
    elif mutation == "native": files["java-inputs/native.bin"] += b"replacement"
    elif mutation == "native_manifest": files["java-inputs/native.json"] += b" "
    elif mutation == "dependency": files["java-inputs/dependencies/0.jar"] += b"replacement"
    elif mutation == "jdk": files["java-inputs/jdk/lib/modules"] += b"replacement"
    elif mutation == "source": members["sources/SorafsReferenceValidatorsJavaConsumerTest.java"] += b"//changed"
    elif mutation == "fixture": members["snapshot/fixtures/sorafs_manifest/fixture"] += b"changed"
    elif mutation == "archive_extra": members["unexpected"] = b"extra"
    elif mutation == "report": members["jvm/junit.xml"] = members["jvm/junit.xml"].replace(b'failures="0"', b'failures="1"')
    elif mutation == "full_stream": members["jvm/execute.log"] = members["jvm/execute.log"].split(producer.REPORT_PREFIX)[0]
    elif mutation == "cases": manifest["executions"][0]["cases"].pop()
    elif mutation == "loaded": manifest["executions"][0]["loaded_classes"].pop()
    elif mutation == "compiled": manifest["executions"][0]["compiled_classes"].clear()
    elif mutation == "missing_probe":
        path = "org/hyperledger/iroha/qualification/SorafsAndroidPackageLinkProbe.class"
        del manifest["executions"][1]["compiled_classes"][path]
        del members["android-host/classes/" + path]
    elif mutation == "split_root":
        for name in tuple(members):
            if name.startswith("android-host/") and name.endswith(".log"):
                members[name] = members[name].replace(str(args.work_dir).encode(), str(args.work_dir.parent / "another").encode())
    elif mutation == "unknown_manifest": manifest["passed"] = True
    elif mutation == "dependency_projection": manifest["executions"][0]["dependency_classes"]["execution"].pop()
    elif mutation == "missing_dependency_projection": del manifest["executions"][0]["dependency_classes"]
    elif mutation == "dependency_origin":
        members["jvm/execute.log"] = members["jvm/execute.log"].replace(b"dependencies/0.jar", b"foreign/0.jar")
        classes, _, _ = producer.consume_runtime_output(members["jvm/execute.log"], report_required=True)
        members["jvm/classes.log"] = classes
    elif mutation in ("runner_origin", "runner_missing", "probe_origin"):
        lane, log, owner = (("android-host", "probe.log", "SorafsAndroidPackageLinkProbe")
                            if mutation == "probe_origin" else ("jvm", "execute.log", "SorafsJavaConsumerQualificationRunner"))
        lines = members[lane + "/" + log].splitlines(keepends=True)
        for position, line in enumerate(lines):
            if ("qualification." + owner + " source:").encode() in line:
                lines[position] = b"" if mutation == "runner_missing" else line.split(b" source:")[0] + b" source: file:///foreign/runner.jar\n"
        members[lane + "/" + log] = b"".join(lines)
        classes, _, _ = producer.consume_runtime_output(members[lane + "/" + log], report_required=log == "execute.log")
        members[lane + ("/probe-classes.log" if log == "probe.log" else "/classes.log")] = classes
    manifest["retained"] = {name: contract.identity(raw) for name, raw in sorted(members.items()) if name != "manifest.json"}
    if mutation == "retained": manifest["retained"].pop(next(iter(manifest["retained"])))
    members["manifest.json"] = contract.canonical_json(manifest)
    files["java_source_kotlin.zip"] = contract.deterministic_archive(members)
    with pytest.raises((ValueError, RuntimeError)):
        verify_fixture(args, value, files, tmp_path / "indexed")
