"""Offline, dependency-free Cargo probes must retain compiler behavior and evidence.

Requires installed Cargo/rustc and POSIX wait4; downloads are never attempted.
All manifests, lockfiles, launchers, source probes, and build outputs are temporary.
No caller RUSTC wrapper, Cargo configuration, or workspace target is reused.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "profile_rustc.py"
SOURCES = {
    "stdin": b"pub fn stdin_probe() -> std::vec::Vec<u8> { std::vec::Vec::new() }\n",
    "package": b"pub fn package_probe() { let unused_probe_value = 42; }\n",
    "generated": b"pub fn generated_probe() -> u32 { 42 }\n",
    "negative": b'compile_error!("legitimate negative capability probe");\n',
}
EXPECTED_CFGS = {
    "probe_stdin_supported", "probe_package_supported",
    "probe_generated_supported", "probe_negative_rejected",
}

BUILD_SCRIPT = r'''//! Direct-RUSTC probes model autocfg, rustix, and file-based feature detection.
use std::{env, fs, io::Write, path::Path, process::{Command, Output, Stdio}};

fn compiler() -> Command {
    assert!(env::var_os("RUSTC_WRAPPER").is_none());
    assert!(env::var_os("RUSTC_WORKSPACE_WRAPPER").is_none());
    Command::new(env::var_os("RUSTC").unwrap())
}

fn stdin_probe(source: &[u8], destination: &Path) -> Output {
    // rustix supplies no crate name and uses -o rather than --out-dir.
    let mut child = compiler()
        .args(["--crate-type=lib", "--emit=metadata", "-o"])
        .arg(destination).arg("-")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

fn main() {
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let discovery = compiler().arg("-vV").output().unwrap();
    assert!(discovery.status.success());
    assert!(String::from_utf8_lossy(&discovery.stdout).contains("release:"));

    let stdin_output = out.join("stdin-probe.rmeta");
    let stdin = stdin_probe(include_bytes!("probes/stdin.rs"), &stdin_output);
    assert!(stdin.status.success(), "{}", String::from_utf8_lossy(&stdin.stderr));
    assert!(stdin_output.is_file());
    fs::remove_file(stdin_output).unwrap();
    println!("cargo:rustc-cfg=probe_stdin_supported");

    // proc-macro2/anyhow/thiserror probe package files, outside OUT_DIR, without
    // Cargo's JSON diagnostics. Preserve the deliberately human warning here.
    let package_output = out.join("package-probe-output");
    fs::create_dir(&package_output).unwrap();
    let package = compiler()
        .args(["--crate-name=package_probe", "--crate-type=lib",
               "--emit=dep-info,metadata", "--error-format=human", "--out-dir"])
        .arg(&package_output).arg("probes/package.rs").output().unwrap();
    assert!(package.status.success(), "{}", String::from_utf8_lossy(&package.stderr));
    assert!(String::from_utf8_lossy(&package.stderr).contains("warning: unused variable"));
    assert!(!String::from_utf8_lossy(&package.stderr).contains("\"$message_type\""));
    assert!(package_output.join("libpackage_probe.rmeta").is_file());
    fs::remove_dir_all(package_output).unwrap();
    println!("cargo:rustc-cfg=probe_package_supported");

    // eyre generates a source file in OUT_DIR and invokes RUSTC directly.
    let generated_source = out.join("generated.rs");
    fs::write(&generated_source, include_bytes!("probes/generated.rs")).unwrap();
    let generated_output = out.join("generated-probe-output");
    fs::create_dir(&generated_output).unwrap();
    let generated = compiler()
        .args(["--crate-name=generated_probe", "--crate-type=lib",
               "--emit=metadata", "--out-dir"])
        .arg(&generated_output).arg(&generated_source).output().unwrap();
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
    assert!(generated_output.join("libgenerated_probe.rmeta").is_file());
    fs::remove_dir_all(generated_output).unwrap();
    fs::remove_file(generated_source).unwrap();
    println!("cargo:rustc-cfg=probe_generated_supported");

    let negative_output = out.join("negative-probe.rmeta");
    let negative = stdin_probe(include_bytes!("probes/negative.rs"), &negative_output);
    assert_eq!(negative.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&negative.stderr)
        .contains("legitimate negative capability probe"));
    assert!(!negative_output.exists());
    println!("cargo:rustc-cfg=probe_negative_rejected");
    for output in [&discovery, &stdin, &package, &generated, &negative] {
        assert!(!String::from_utf8_lossy(&output.stderr).contains("jobserver"),
            "nested compiler lost jobserver descriptors: {}",
            String::from_utf8_lossy(&output.stderr));
    }
    println!("cargo:rerun-if-changed=build.rs");
    for name in ["stdin", "package", "generated", "negative"] {
        println!("cargo:rerun-if-changed=probes/{}.rs", name);
    }
    for name in ["probe_stdin_supported", "probe_package_supported",
                 "probe_generated_supported", "probe_negative_rejected"] {
        println!("cargo:rustc-check-cfg=cfg({})", name);
    }
}
'''


def installed_compiler_tools() -> tuple[str, str]:
    """Resolve installed toolchain executables without invoking an installer."""
    cargo, rustc = shutil.which("cargo"), shutil.which("rustc")
    if not cargo or not rustc or not hasattr(os, "wait4"):
        pytest.skip("real probe qualification requires installed Cargo/rustc and wait4")
    rustup = shutil.which("rustup")
    if rustup:
        resolved = []
        for name in ("cargo", "rustc"):
            result = subprocess.run(
                [rustup, "which", name], capture_output=True, text=True, timeout=30,
            )
            assert result.returncode == 0, result.stderr
            path = Path(result.stdout.strip()).resolve(strict=True)
            assert path.is_file()
            resolved.append(str(path))
        return resolved[0], resolved[1]
    return str(Path(cargo).resolve(strict=True)), str(Path(rustc).resolve(strict=True))


@pytest.fixture(scope="module")
def real_probe_build(tmp_path_factory: pytest.TempPathFactory):
    """Compare cold ordinary/instrumented builds of one standalone Cargo package."""
    cargo, rustc = installed_compiler_tools()
    root = tmp_path_factory.mktemp("profile-build-script-probes").resolve()
    source = root / "source"
    source.mkdir()
    (source / "probes").mkdir()
    for name, content in SOURCES.items():
        (source / "probes" / f"{name}.rs").write_bytes(content)
    (source / "Cargo.toml").write_text(
        '[package]\nname="profile_probe_fixture"\nversion="0.1.0"\nedition="2021"\n'
        '[workspace]\n[lib]\npath="lib.rs"\n[profile.dev]\ndebug=0\n',
        encoding="utf-8",
    )
    lock = source / "Cargo.lock"
    lock.write_text(
        'version = 3\n\n[[package]]\nname = "profile_probe_fixture"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    initial_lock = lock.read_bytes()
    (source / "build.rs").write_text(BUILD_SCRIPT, encoding="utf-8")
    (source / "lib.rs").write_text(
        '//! All supported compiler probes must affect the compiled library.\n'
        '#[cfg(not(all(probe_stdin_supported, probe_package_supported, '
        'probe_generated_supported, probe_negative_rejected)))]\n'
        'compile_error!("profiler changed compiler capability detection");\n'
        'pub fn answer() -> u32 { 42 }\n', encoding="utf-8",
    )
    # Bind this entire fixture to one helper revision even if another test is
    # editing the repository after collection.
    helper = root / "profile_rustc.py"
    shutil.copyfile(SCRIPT, helper)
    spec = importlib.util.spec_from_file_location("profile_build_script_probe_subject", helper)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    records_directory = root / "records"
    records_directory.mkdir()
    launcher = root / "measured-rustc"
    launcher.write_text(
        "#!/bin/sh\nexec " + " ".join(shlex.quote(value) for value in (
            sys.executable, "-I", "-S", str(helper), "--record-dir", str(records_directory),
            "--expected-compiler", rustc, rustc,
        )) + ' "$@"\n', encoding="utf-8",
    )
    launcher.chmod(0o700)
    cargo_home = root / "cargo-home"
    cargo_home.mkdir()
    environment = {
        key: value for key, value in os.environ.items()
        if not key.startswith(("CARGO_", "RUSTC", "RUSTFLAGS", "RUSTDOC", "RUSTUP_"))
    }
    environment.update(CARGO_HOME=str(cargo_home), CARGO_NET_OFFLINE="true", CARGO_INCREMENTAL="0")

    def build(label: str, compiler: str):
        target = root / label
        result = subprocess.run(
            [cargo, "build", "--locked", "--offline", "--jobs=1", "--message-format=json"],
            cwd=source, env=dict(environment, RUSTC=compiler, CARGO_TARGET_DIR=str(target)),
            capture_output=True, text=True, timeout=120,
        )
        assert result.returncode == 0, result.stderr
        assert "failed to connect to jobserver" not in result.stderr
        assert lock.read_bytes() == initial_lock
        return target, [json.loads(line) for line in result.stdout.splitlines()]

    _, ordinary = build("ordinary-target", rustc)
    target, messages = build("measured-target", str(launcher))
    records = module.read_records(records_directory)
    artifacts = [value for value in messages if value.get("reason") == "compiler-artifact"]
    scripts = [value for value in messages if value.get("reason") == "build-script-executed"]
    evidence = module.reconcile_measurements(
        artifacts, records, {"source": source, "target": target},
        lambda value: {key: value[key] for key in ("package_id", "target", "features", "profile")},
        build_script_messages=scripts,
    )
    return {
        "source": source, "target": target, "records": records, "evidence": evidence,
        "ordinary": ordinary, "scripts": scripts, "artifacts": artifacts,
    }


def test_direct_rustc_probe_results_match_an_ordinary_offline_build(real_probe_build) -> None:
    """The library compiles only when every real capability result survives."""
    ordinary = [value for value in real_probe_build["ordinary"]
                if value.get("reason") == "build-script-executed"]
    measured = real_probe_build["scripts"]
    assert len(ordinary) == len(measured) == 1
    assert set(ordinary[0]["cfgs"]) == set(measured[0]["cfgs"]) == EXPECTED_CFGS
    evidence = real_probe_build["evidence"]
    assert len(evidence["compiled"]) == len(real_probe_build["artifacts"]) == 2
    assert evidence["fresh"] == evidence["failed"] == []
    assert all(unit["peak_rss_bytes"] > 0 for unit in evidence["compiled"])


@pytest.mark.parametrize("name", list(SOURCES))
def test_each_direct_probe_has_exact_source_identity_and_owned_metrics(real_probe_build, name: str) -> None:
    """Record generated bytes once, including a legitimate compiler rejection."""
    content = SOURCES[name]
    digest = hashlib.sha256(content).hexdigest()
    records = [record for record in real_probe_build["records"]
               if record["probe"] and record["probe_source"]
               and record["probe_source"]["sha256"] == digest]
    assert len(records) == 1
    record = records[0]
    expected_path = None
    kind = "stdin"
    if name == "package":
        kind, expected_path = "file", str(real_probe_build["source"] / "probes/package.rs")
    elif name == "generated":
        kind = "file"
        expected_path = str(Path(real_probe_build["scripts"][0]["out_dir"]) / "generated.rs")
    assert record["probe_source"] == {
        "kind": kind, "path": expected_path, "bytes": len(content), "sha256": digest,
    }
    assert record["returncode"] == (1 if name == "negative" else 0)
    assert record["peak_rss_bytes"] > 0
    assert record["elapsed_ns"] > 0
    assert not any(arg.startswith("--json") or arg == "--error-format=json"
                   for arg in record["arguments"])
    if kind == "stdin":
        assert "-" in record["arguments"] and "-o" in record["arguments"]
        assert not any(arg.startswith("--crate-name") for arg in record["arguments"])
    evidence, = [probe for probe in real_probe_build["evidence"]["probes"]
                 if probe["source"] and probe["source"]["sha256"] == digest]
    projected_path = None
    if name == "package":
        projected_path = "source/probes/package.rs"
    elif expected_path:
        projected_path = "target/" + str(Path(expected_path).relative_to(real_probe_build["target"]))
    assert evidence["source"] == {**record["probe_source"], "path": projected_path}
    assert evidence["returncode"] == record["returncode"]
    assert evidence["peak_rss_bytes"] == record["peak_rss_bytes"]


def test_probe_cleanup_does_not_create_missing_cargo_artifact_failures(real_probe_build) -> None:
    """Probe outputs/source can disappear before Cargo's artifact reconciliation."""
    output = Path(real_probe_build["scripts"][0]["out_dir"])
    for name in ("stdin-probe.rmeta", "negative-probe.rmeta", "package-probe-output",
                 "generated-probe-output", "generated.rs"):
        assert not (output / name).exists()
    source_probes = [value for value in real_probe_build["evidence"]["probes"] if value["source"]]
    assert len(source_probes) == 4
    assert sum(value["returncode"] != 0 for value in source_probes) == 1


def test_direct_compiler_discovery_is_measured_separately_from_source(real_probe_build) -> None:
    """The build script's -vV result remains discovery, without invented bytes."""
    discovery = [value for value in real_probe_build["evidence"]["probes"]
                 if value["source"] is None and value["arguments"] == ["-vV"]]
    assert discovery
    assert all(value["returncode"] == 0 and value["peak_rss_bytes"] > 0 for value in discovery)
    assert all(value["compiler_artifacts"] == [] for value in discovery)
