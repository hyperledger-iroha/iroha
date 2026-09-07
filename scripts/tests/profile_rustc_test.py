"""Compiler-process resource measurement and Cargo artifact identity tests."""

from __future__ import annotations

import copy
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "profile_rustc.py"
SPEC = importlib.util.spec_from_file_location("profile_rustc_test_subject", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def fake_compiler(path: Path) -> None:
    """Write a compiler that forwards diagnostics, allocates RAM, and emits a file."""
    path.write_text(
        f"#!{sys.executable}\n"
        "import json, os, pathlib, sys\n"
        "if os.environ.get('FIXTURE_JOBSERVER'):\n"
        "    read_fd, write_fd, unrelated = map(int, os.environ['FIXTURE_JOBSERVER'].split(','))\n"
        "    os.write(write_fd, b'token'); assert os.read(read_fd, 5) == b'token'\n"
        "    try: os.fstat(unrelated)\n"
        "    except OSError: pass\n"
        "    else: raise AssertionError('unrelated descriptor leaked to compiler')\n"
        "if '-Vv' in sys.argv:\n"
        "    print('fake rustc 1.0'); sys.exit(0)\n"
        "memory = bytearray(int(os.environ.get('FIXTURE_MEMORY_MIB', '1')) * 1024**2)\n"
        "print('compiler diagnostic', file=sys.stderr)\n"
        "if os.environ.get('FIXTURE_FAIL'):\n"
        "    sys.exit(7)\n"
        "out = pathlib.Path(sys.argv[sys.argv.index('--out-dir') + 1]) / 'libfixture.rlib'\n"
        "out.parent.mkdir(parents=True, exist_ok=True)\n"
        "out.write_bytes(b'compiled artifact')\n"
        "print(json.dumps({'artifact': str(out), 'emit': 'link'}), file=sys.stderr)\n",
        encoding="utf-8",
    )
    path.chmod(0o700)


def invoke_fixture(tmp_path: Path, *, memory: int = 1, fail: bool = False, descriptors=()):
    """Execute the actual wrapper against a bounded fake compiler."""
    source = tmp_path / "source.rs"
    source.write_text("//! Fixture.\n", encoding="utf-8")
    (tmp_path / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
    compiler = tmp_path / "rustc"
    fake_compiler(compiler)
    records = tmp_path / "records"
    records.mkdir()
    target = tmp_path / "target"
    arguments = ["--crate-name", "fixture", str(source), "--error-format=json",
                 "--json=diagnostic-rendered-ansi", "--out-dir", str(target),
                 "--cfg", 'feature="z"', "--cfg=feature=\"a\"",
                 "-C", "opt-level=1", "-C", "debug-assertions=off", "-C", "overflow-checks=on"]
    environment = dict(os.environ, CARGO_MANIFEST_DIR=str(tmp_path), FIXTURE_MEMORY_MIB=str(memory))
    if fail:
        environment["FIXTURE_FAIL"] = "true"
    if descriptors:
        read_fd, write_fd, unrelated = descriptors
        environment["CARGO_MAKEFLAGS"] = f"-j --jobserver-fds={read_fd},{write_fd} --jobserver-auth={read_fd},{write_fd}"
        environment["FIXTURE_JOBSERVER"] = f"{read_fd},{write_fd},{unrelated}"
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--record-dir", str(records),
         "--expected-compiler", str(compiler), str(compiler), *arguments],
        env=environment, cwd=tmp_path, capture_output=True, text=True, pass_fds=descriptors,
    )
    return result, MODULE.read_records(records)


def test_actual_compiler_child_inherits_jobserver_and_no_unrelated_descriptor(tmp_path: Path) -> None:
    read_fd, write_fd = os.pipe()
    unrelated = os.open(tmp_path, os.O_RDONLY)
    try:
        result, records = invoke_fixture(tmp_path, descriptors=(read_fd, write_fd, unrelated))
        assert result.returncode == 0, result.stderr
        assert len(records) == 1
    finally:
        for descriptor in (read_fd, write_fd, unrelated):
            os.close(descriptor)


@pytest.mark.parametrize("flags", ["-j", "-j --jobserver-auth=fifo:/unused/path", ""])
def test_jobserver_without_pipe_needs_no_open_handles(flags: str) -> None:
    assert MODULE.jobserver_descriptors({"CARGO_MAKEFLAGS": flags}) == ()


@pytest.mark.parametrize("flags", [
    "--jobserver-auth", "--jobserver-auth=", "--jobserver-auth=-1,-1",
    "--jobserver-auth=3", "--jobserver-auth=3,x", "--jobserver-auth=3,3",
    "--jobserver-auth=0,1", "--jobserver-auth=３,４", "--jobserver-auth=fifo:",
    "--jobserver-auth=3,4 --jobserver-fds=4,5",
    "--jobserver-auth=fifo:a --jobserver-fds=3,4",
    "--jobserver-auth=fifo:a --jobserver-auth=fifo:b", "--jobserver-auth=999998,999999",
])
def test_jobserver_rejects_malformed_conflicting_or_closed_handles(flags: str) -> None:
    with pytest.raises(ValueError, match="jobserver"):
        MODULE.jobserver_descriptors({"CARGO_MAKEFLAGS": flags})


@pytest.mark.parametrize("platform,raw,expected", [("darwin", 19, 19), ("linux", 19, 19 * 1024)])
def test_rss_normalization(platform: str, raw: int, expected: int) -> None:
    assert MODULE.peak_rss_bytes(raw, platform) == expected


@pytest.mark.parametrize("platform,raw", [("win32", 19), ("linux", 0), ("darwin", True)])
def test_rss_rejects_unavailable_measurements(platform: str, raw: int) -> None:
    with pytest.raises(ValueError):
        MODULE.peak_rss_bytes(raw, platform)


@pytest.mark.parametrize("fail", [False, True])
def test_real_child_wait4_preserves_diagnostics_exit_and_invocation_identity(tmp_path: Path, fail: bool) -> None:
    result, records = invoke_fixture(tmp_path, fail=fail)
    assert result.returncode == (7 if fail else 0), result.stderr
    assert "compiler diagnostic" in result.stderr
    assert len(records) == 1
    record = records[0]
    assert record["returncode"] == result.returncode
    assert record["peak_rss_bytes"] > 0
    assert record["elapsed_ns"] > 0
    assert record["features"] == ["a", "z"]
    assert record["manifest_path"] == str(tmp_path / "Cargo.toml")
    assert "--json=diagnostic-rendered-ansi,artifacts" in record["arguments"]
    assert bool(record["artifacts"]) is not fail


def test_rss_is_per_invocation_not_an_earlier_child_high_water_mark(tmp_path: Path) -> None:
    big = tmp_path / "big"
    small = tmp_path / "small"
    big.mkdir()
    small.mkdir()
    big_result, big_records = invoke_fixture(big, memory=96)
    small_result, small_records = invoke_fixture(small, memory=1)
    assert big_result.returncode == small_result.returncode == 0
    assert big_records[0]["peak_rss_bytes"] > small_records[0]["peak_rss_bytes"] + 32 * 1024**2


def test_probe_does_not_change_compiler_discovery_arguments(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("CARGO_MANIFEST_DIR", raising=False)
    for arguments in (["-Vv"], ["-", "--crate-name", "___", "--print=file-names"]):
        assert MODULE.invocation_identity(arguments, Path.cwd()) == {"probe": True, "probe_source": None}
        assert MODULE.artifact_diagnostics(arguments, True) == arguments


@pytest.mark.parametrize("source_kind", ["stdin", "file"])
@pytest.mark.parametrize("valid", [True, False])
def test_real_source_probe_preserves_compiler_answer_and_binds_input(tmp_path: Path, source_kind: str, valid: bool) -> None:
    """autocfg's std detection sees the real compiler, including negative probes."""
    rustc = shutil.which("rustc")
    if not rustc:
        pytest.skip("source probe qualification requires rustc")
    root = tmp_path.resolve()
    output = root / "out"
    output.mkdir()
    records = root / "records"
    records.mkdir()
    source = b"pub fn probe() -> std::vec::Vec<u8> { std::vec::Vec::new() }\n" if valid else b'compile_error!("unsupported probe");\n'
    path = output / "probe.rs"
    path.write_bytes(source)
    arguments = ["--crate-name", "autocfg_probe", "--crate-type=lib", "--out-dir", str(output),
                 "--emit=llvm-ir", "-" if source_kind == "stdin" else str(path)]
    environment = dict(os.environ, OUT_DIR=str(output), CARGO_MANIFEST_DIR=str(root))
    expected = subprocess.run([rustc, *arguments], input=source if source_kind == "stdin" else None,
                              env=environment, capture_output=True, timeout=60)
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--record-dir", str(records),
         "--expected-compiler", rustc, rustc, *arguments],
        input=source if source_kind == "stdin" else None, env=environment,
        capture_output=True, timeout=60,
    )
    assert result.returncode == expected.returncode == (0 if valid else 1), result.stderr
    assert result.stdout == expected.stdout
    assert result.stderr == expected.stderr
    record, = MODULE.read_records(records)
    assert record["probe"] is True
    assert record["arguments"] == arguments
    assert record["probe_source"] == {
        "kind": source_kind, "path": str(path) if source_kind == "file" else None,
        "bytes": len(source), "sha256": hashlib.sha256(source).hexdigest(),
    }
    assert record["peak_rss_bytes"] > 0
    assert record["returncode"] == result.returncode


def test_unclassified_invocation_persists_fatal_instrumentation_evidence(tmp_path: Path) -> None:
    """A build script ignoring the wrapper's failure cannot produce valid evidence."""
    compiler = tmp_path / "rustc"
    fake_compiler(compiler)
    directory = tmp_path / "records"
    directory.mkdir()
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--record-dir", str(directory),
         "--expected-compiler", str(compiler), str(compiler), "--unsupported-fixture"],
        capture_output=True, text=True,
    )
    assert result.returncode == 2
    with pytest.raises(ValueError, match="instrumentation failed"):
        MODULE.read_records(directory)


def test_compiler_json_modes_are_preserved_and_artifacts_not_duplicated() -> None:
    arguments = ["--error-format", "json", "--json", "artifacts,diagnostic-short"]
    assert MODULE.artifact_diagnostics(arguments, False) == arguments
    assert MODULE.artifact_diagnostics(["--error-format=json"], False)[-1] == "--json=artifacts"
    with pytest.raises(ValueError, match="JSON diagnostics"):
        MODULE.artifact_diagnostics(["--error-format=human"], False)


def reconciliation_fixture(tmp_path: Path):
    result, records = invoke_fixture(tmp_path)
    assert result.returncode == 0, result.stderr
    record = records[0]
    message = {
        "reason": "compiler-artifact", "package_id": "workspace#fixture@1.0.0",
        "manifest_path": str(tmp_path / "Cargo.toml"),
        "target": {"name": "fixture", "src_path": str(tmp_path / "source.rs"), "kind": ["lib"], "crate_types": ["lib"]},
        "features": ["a", "z"],
        "profile": {"opt_level": "1", "debuginfo": 0, "debug_assertions": False, "overflow_checks": True, "test": False},
        "filenames": [record["artifacts"][0]["path"]], "fresh": False,
    }
    roots = {"source": tmp_path, "target": tmp_path / "target"}
    project = lambda value: {key: copy.deepcopy(value[key]) for key in ("package_id", "target", "features", "profile")}
    return message, record, roots, project


def test_reconciliation_binds_full_unit_and_artifact_identity(tmp_path: Path) -> None:
    message, record, roots, project = reconciliation_fixture(tmp_path)
    result = MODULE.reconcile_measurements([message], [record], roots, project)
    compiled = result["compiled"][0]
    assert compiled["unit"] == project(message)
    assert compiled["peak_rss_bytes"] == record["peak_rss_bytes"]
    assert compiled["cargo_artifacts"] == compiled["compiler_artifacts"]
    assert compiled["cargo_artifacts"][0]["path"] == "target/libfixture.rlib"
    assert str(tmp_path) not in json.dumps(compiled["arguments"])
    assert result["fresh"] == result["failed"] == result["probes"] == []


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "orphan", "feature", "source", "manifest", "test", "name", "artifact", "rss", "profile"])
def test_missing_or_mismatched_evidence_fails_closed(tmp_path: Path, mutation: str) -> None:
    message, record, roots, project = reconciliation_fixture(tmp_path)
    records = [record]
    messages = [message]
    if mutation == "missing":
        records = []
    elif mutation == "duplicate":
        records.append(copy.deepcopy(record))
    elif mutation == "orphan":
        messages = []
    elif mutation in ("feature", "test", "name"):
        if mutation == "feature":
            record["features"] = []
        elif mutation == "test":
            record["profile"]["test"] = True
        else:
            record["crate_name"] = "different"
    elif mutation in ("source", "manifest"):
        record[mutation + "_path"] = str(tmp_path / "other.rs")
    elif mutation == "profile":
        message["profile"]["opt_level"] = "3"
    elif mutation == "artifact":
        record["artifacts"][0]["sha256"] = "f" * 64
    elif mutation == "rss":
        record["peak_rss_bytes"] = 0
        directory = tmp_path / "bad-records"
        directory.mkdir()
        MODULE.write_record(directory, record)
        with pytest.raises(ValueError, match="peak_rss"):
            MODULE.read_records(directory)
        return
    with pytest.raises(ValueError):
        MODULE.reconcile_measurements(messages, records, roots, project)


def test_warm_artifacts_have_no_fabricated_rss(tmp_path: Path) -> None:
    message, _, roots, project = reconciliation_fixture(tmp_path)
    message["fresh"] = True
    result = MODULE.reconcile_measurements([message], [], roots, project)
    assert result["compiled"] == []
    assert result["fresh"] == [{"unit": project(message), "artifacts": result["fresh"][0]["artifacts"]}]
    assert "peak_rss_bytes" not in result["fresh"][0]


def test_record_reader_rejects_symlinks_and_duplicate_keys(tmp_path: Path) -> None:
    outside = tmp_path / "outside"
    outside.write_text('{}', encoding="utf-8")
    directory = tmp_path / "records"
    directory.mkdir()
    (directory / "record.json").symlink_to(outside)
    with pytest.raises((OSError, ValueError)):
        MODULE.read_records(directory)
    (directory / "record.json").unlink()
    (directory / "record.json").write_text('{"probe":true,"probe":false}', encoding="utf-8")
    with pytest.raises(ValueError, match="duplicate"):
        MODULE.read_records(directory)


@pytest.mark.parametrize(
    "arguments,expected",
    [
        ([], {"opt_level": "0", "debuginfo": 0, "debug_assertions": True, "overflow_checks": True, "test": False}),
        (["-Copt-level=3"], {"opt_level": "3", "debuginfo": 0, "debug_assertions": False, "overflow_checks": False, "test": False}),
        (["-C", "opt-level=1", "-Cdebug-assertions=on", "-Coverflow-checks=off", "-Cdebuginfo=line-tables-only", "--test"],
         {"opt_level": "1", "debuginfo": "line-tables-only", "debug_assertions": True, "overflow_checks": False, "test": True}),
    ],
)
def test_compiler_profile_defaults_and_explicit_codegen_flags(arguments, expected) -> None:
    assert MODULE.compiler_profile(arguments) == expected


def test_artifact_hashing_rejects_symlinked_ancestors(tmp_path: Path) -> None:
    directory = tmp_path / "real"
    directory.mkdir()
    (directory / "file").write_bytes(b"artifact")
    (tmp_path / "link").symlink_to(directory, target_is_directory=True)
    with pytest.raises(OSError):
        MODULE.stable_file_identity(tmp_path / "link" / "file")


def test_cargo_executable_copy_matches_compiler_artifact_bytes(tmp_path: Path) -> None:
    message, record, roots, project = reconciliation_fixture(tmp_path)
    published = tmp_path / "target" / "published-copy"
    published.write_bytes(Path(record["artifacts"][0]["path"]).read_bytes())
    message["filenames"] = [str(published)]
    result = MODULE.reconcile_measurements([message], [record], roots, project)
    assert result["compiled"][0]["cargo_artifacts"][0]["path"] == "target/published-copy"
    assert result["compiled"][0]["compiler_artifacts"][0]["path"] == "target/libfixture.rlib"


@pytest.mark.parametrize("cargo_arguments", [["build"], ["build", "--release"], ["test", "--no-run"], ["check"]])
def test_real_cargo_artifacts_reconcile_build_scripts_libraries_and_test_targets(tmp_path: Path, cargo_arguments) -> None:
    """Exercise rustc/Cargo's actual JSON contract, including copied executables."""
    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    if not cargo or not rustc:
        pytest.skip("real compiler artifact qualification requires Cargo and rustc")
    root = tmp_path.resolve()
    source = root / "source"
    source.mkdir()
    (source / "Cargo.toml").write_text(
        '[package]\nname="fixture"\nversion="0.1.0"\nedition="2024"\n'
        '[lib]\npath="lib.rs"\n[features]\nselected=[]\n'
        '[profile.dev]\ndebug=0\n[profile.release]\ndebug="line-tables-only"\n',
        encoding="utf-8",
    )
    (source / "lib.rs").write_text('//! Fixture.\npub fn answer() -> u32 { 42 }\n#[test] fn check() { assert_eq!(answer(), 42); }\n', encoding="utf-8")
    (source / "build.rs").write_text(r'''//! Build fixture.
use std::{io::Write, process::{Command, Stdio}};
fn main() {
    let mut child = Command::new(std::env::var_os("RUSTC").unwrap())
        .args(["--crate-name", "autocfg_std_probe", "--crate-type=lib", "--emit=llvm-ir", "--out-dir"])
        .arg(std::env::var_os("OUT_DIR").unwrap()).arg("-")
        .stdin(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"pub fn probe() -> std::vec::Vec<u8> { std::vec::Vec::new() }").unwrap();
    assert!(child.wait().unwrap().success(), "std probe must see the real compiler");
    println!("cargo::rustc-cfg=feature=\"injected\"");
}
''', encoding="utf-8")
    records = root / "records"
    records.mkdir()
    target = root / "target"
    wrapper = root / "wrapper"
    wrapper.write_text(
        "#!/bin/sh\nexec " + " ".join(shlex.quote(value) for value in (
            sys.executable, "-I", "-S", str(SCRIPT), "--record-dir", str(records),
            "--expected-compiler", rustc, rustc,
        )) + ' "$@"\n', encoding="utf-8",
    )
    wrapper.chmod(0o700)
    environment = dict(os.environ, RUSTC=str(wrapper),
                       CARGO_TARGET_DIR=str(target), CARGO_INCREMENTAL="0")
    environment.pop("RUSTC_WORKSPACE_WRAPPER", None)
    environment.pop("RUSTC_WRAPPER", None)
    result = subprocess.run(
        [cargo, *cargo_arguments, "--offline", "--features=selected", "--jobs=2", "--message-format=json"],
        cwd=source, env=environment, capture_output=True, text=True, timeout=120,
    )
    assert result.returncode == 0, result.stderr
    assert "failed to connect to jobserver" not in result.stderr
    messages = [json.loads(line) for line in result.stdout.splitlines()]
    artifacts = [message for message in messages if message.get("reason") == "compiler-artifact"]
    evidence = MODULE.reconcile_measurements(
        artifacts, MODULE.read_records(records), {"source": source, "target": target},
        lambda value: {key: value[key] for key in ("package_id", "target", "features", "profile")},
        build_script_messages=[message for message in messages if message.get("reason") == "build-script-executed"],
    )
    assert len(evidence["compiled"]) == len(artifacts) >= 2
    source_probes = [probe for probe in evidence["probes"] if probe["source"] is not None]
    assert len(source_probes) == 1
    assert source_probes[0]["source"]["kind"] == "stdin"
    assert source_probes[0]["returncode"] == 0
    assert all(value["peak_rss_bytes"] > 0 for value in evidence["compiled"])
    for compiled in evidence["compiled"]:
        assert compiled["unit"]["features"] == ["selected"]
        if compiled["unit"]["target"]["kind"] != ["custom-build"]:
            assert compiled["build_script"]["cfgs"] == ['feature="injected"']


@pytest.mark.parametrize("mutation", ["none", "missing", "package", "directory", "feature", "conflict", "escape", "malformed"])
def test_build_script_cfg_features_require_matching_cargo_evidence(tmp_path: Path, mutation: str) -> None:
    message, record, roots, project = reconciliation_fixture(tmp_path)
    directory = tmp_path / "target" / "build" / "fixture" / "out"
    directory.mkdir(parents=True)
    record["build_script_out_dir"] = str(directory)
    record["features"].append("injected")
    record["features"].sort()
    script = {"reason": "build-script-executed", "package_id": message["package_id"],
              "out_dir": str(directory), "cfgs": ['feature="injected"']}
    scripts = [script]
    if mutation == "missing":
        scripts = []
    elif mutation == "package":
        script["package_id"] = "workspace#different@1.0.0"
    elif mutation == "directory":
        other = directory.parent / "other"
        other.mkdir()
        script["out_dir"] = str(other)
    elif mutation == "feature":
        script["cfgs"] = ['feature="different"']
    elif mutation == "conflict":
        scripts.append({**script, "cfgs": ['feature="different"']})
    elif mutation == "escape":
        script["out_dir"] = str(tmp_path)
    elif mutation == "malformed":
        script["cfgs"] = ['feature=broken']
    if mutation != "none":
        with pytest.raises(ValueError):
            MODULE.reconcile_measurements([message], [record], roots, project, build_script_messages=scripts)
        return
    result = MODULE.reconcile_measurements([message], [record], roots, project, build_script_messages=scripts)
    compiled = result["compiled"][0]
    assert compiled["unit"]["features"] == ["a", "z"]
    assert compiled["build_script"]["cfgs"] == ['feature="injected"']
    assert compiled["build_script"]["out_dir"] == "target/build/fixture/out"
