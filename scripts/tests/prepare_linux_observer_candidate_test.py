"""Protocol, retained-original and terminal-observation controls; no Rust builds."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
from contextlib import contextmanager

import pytest

import prepare_linux_observer_candidate as candidate
import workspace_source_keeper as custody


def acknowledgement(directory):
    return (f"bridge_dir\t{directory}\nmodule_sha256\t{'a' * 64}\n"
            f"provenance_sha256\t{'b' * 64}\n").encode()


def test_acknowledgement_requires_order_extent_digests_and_exact_directory(tmp_path):
    expected = tmp_path.resolve() / "bridge"
    valid = acknowledgement(expected)
    assert candidate.acknowledgement(valid, expected)["module_sha256"] == "a" * 64
    mutations = [valid[:-1], valid + b"\n", valid.replace(b"module_sha256", b"provenance_sha256"),
                 valid.replace(b"a" * 64, b"A" * 64), valid.replace(b"b" * 64, b"b" * 63),
                 valid.replace(str(expected).encode(), str(expected / "other").encode()),
                 valid.replace(b"bridge_dir\t", b"bridge_dir\t\t"), b"x" * 65537]
    for bad in mutations:
        with pytest.raises(ValueError):
            candidate.acknowledgement(bad, expected)


def test_paths_refuse_aliases_and_nested_outputs(tmp_path):
    root = tmp_path.resolve()
    real = root / "real"
    real.mkdir()
    alias = root / "alias"
    alias.symlink_to(real, target_is_directory=True)
    with pytest.raises(ValueError, match="canonical"):
        candidate.canonical(str(alias))
    with pytest.raises(ValueError, match="overlap"):
        candidate.disjoint([root, real])
    candidate.disjoint([real, root / "separate"])


def test_publisher_original_stays_open_and_mutation_is_rejected(tmp_path):
    path = tmp_path.resolve() / "publisher"
    path.write_bytes(b"publisher candidate")
    path.chmod(0o700)
    with candidate.held_publisher(custody, path) as (program, descriptor, observed):
        assert program == f"/proc/self/fd/{descriptor}"
        assert observed == hashlib.sha256(path.read_bytes()).hexdigest()
    with pytest.raises(ValueError, match="changed"):
        with candidate.held_publisher(custody, path):
            path.write_bytes(b"changed original")


def test_interrupted_wait_reaps_original_before_unwinding(tmp_path, monkeypatch):
    root = tmp_path.resolve()
    marker = root / "child-complete"
    real_popen = subprocess.Popen
    children = []

    def popen(*args, **kwargs):
        child = real_popen(*args, **kwargs)
        children.append(child)
        real_wait = child.wait
        calls = 0

        def wait(*args, **kwargs):
            nonlocal calls
            calls += 1
            if calls == 1:
                raise RuntimeError("interrupted terminal observation")
            return real_wait(*args, **kwargs)

        child.wait = wait
        return child

    monkeypatch.setattr(candidate.subprocess, "Popen", popen)
    with pytest.raises(RuntimeError, match="interrupted terminal"):
        candidate.run_logged([sys.executable, "-c",
                              "import pathlib,sys,time; time.sleep(.05); pathlib.Path(sys.argv[1]).write_text('done')",
                              str(marker)], root, {}, root, "child")
    assert marker.read_text() == "done"
    assert children[0].returncode == 0


def test_failed_child_retains_logs_and_refuses_success(tmp_path):
    root = tmp_path.resolve()
    with pytest.raises(ValueError, match="exit 7"):
        candidate.run_logged([sys.executable, "-c", "import sys; print('diagnostic'); sys.exit(7)"],
                             root, {}, root, "failed")
    assert (root / "failed.stdout").read_text().strip() == "diagnostic"
    with pytest.raises(FileExistsError):
        candidate.run_logged([sys.executable, "-c", "pass"], root, {}, root, "failed")


def fixture(root):
    bridge = root / "published"
    bridge.mkdir()
    module = b"candidate bytes; never executed by this control"
    (bridge / candidate.LIBRARY).write_bytes(module)
    module_hash = hashlib.sha256(module).hexdigest()
    provenance = {"schema": "iroha.publisher.v1", "source_manifest_sha256": "c" * 64,
                  "native_target": candidate.NATIVE_TARGET, "executable_filename": candidate.LIBRARY,
                  "executable_sha256": module_hash, "executable_bytes": len(module)}
    raw = json.dumps(provenance).encode()
    (bridge / "publisher-source.json").write_bytes(raw)
    return bridge, {"module_sha256": module_hash, "provenance_sha256": hashlib.sha256(raw).hexdigest()}


def test_copy_preserves_exact_two_originals_without_loading(tmp_path):
    root = tmp_path.resolve()
    bridge, pins = fixture(root)
    destination = root / "review"
    evidence = candidate.copy_candidate(custody, bridge, destination, pins, "c" * 64)
    assert evidence["native_target"] == candidate.NATIVE_TARGET
    assert set(p.name for p in destination.iterdir()) == {candidate.LIBRARY, "publisher-source.json"}
    for name in (candidate.LIBRARY, "publisher-source.json"):
        assert (destination / name).read_bytes() == (bridge / name).read_bytes()


@pytest.mark.parametrize("mutation", ["extra", "module", "source", "symlink"])
def test_copy_rejects_extra_tampered_and_misbound_artifacts(tmp_path, mutation):
    root = tmp_path.resolve()
    bridge, pins = fixture(root)
    source = "c" * 64
    if mutation == "extra":
        (bridge / "extra").write_bytes(b"unexpected")
    elif mutation == "module":
        (bridge / candidate.LIBRARY).write_bytes(b"substituted")
    elif mutation == "source":
        source = "d" * 64
    else:
        (bridge / candidate.LIBRARY).rename(root / "outside")
        (bridge / candidate.LIBRARY).symlink_to(root / "outside")
    with pytest.raises((ValueError, custody.SourceCustodyError)):
        candidate.copy_candidate(custody, bridge, root / "review", pins, source)


def test_helper_code_is_checked_before_execution(tmp_path):
    root = tmp_path.resolve()
    (root / "scripts").mkdir()
    (root / "scripts/compute_workspace_source_manifest.py").write_text("raise AssertionError('must never execute')\n")
    with pytest.raises(ValueError, match="differs from fixed"):
        candidate.reviewed_helper(root)


def test_ambiguous_provenance_is_rejected_even_if_candidate_hash_matches(tmp_path):
    root = tmp_path.resolve()
    bridge, pins = fixture(root)
    path = bridge / "publisher-source.json"
    raw = path.read_bytes().replace(b'{', b'{"schema":"conflicting",', 1)
    path.write_bytes(raw)
    pins["provenance_sha256"] = hashlib.sha256(raw).hexdigest()
    with pytest.raises(ValueError, match="repeats a field"):
        candidate.copy_candidate(custody, bridge, root / "review", pins, "c" * 64)


@pytest.mark.parametrize("final_source_failure", [False, True])
def test_prepare_keeps_source_owner_until_copy_and_receipt_follows_final_validation(
        tmp_path, monkeypatch, final_source_failure):
    root = tmp_path.resolve()
    source, output, cargo_home = root / "source", root / "output", root / "cargo-home"
    source.mkdir()
    cargo_home.mkdir()
    (source / "Cargo.lock").write_text("retained lock")
    policy_path = source / candidate.POLICY_PATH
    policy_path.parent.mkdir(parents=True)
    policy_path.write_text('{"allowed": ["PATH"]}')
    (source / "scripts").mkdir()
    (source / "scripts/workspace_source_keeper.py").write_text("from workspace_source_keeper import SourceKeeper, _PosixOriginal, _stamp\n")
    cargo, rustc = root / "cargo", root / "rustc"
    cargo.write_bytes(b"cargo original")
    rustc.write_bytes(b"rustc original")
    state = {"active": False, "validated": 0}

    def validate():
        assert state["active"]
        state["validated"] += 1

    @contextmanager
    def source_owner(*args, **kwargs):
        state["active"] = True
        try:
            yield SimpleNamespace(revalidate=validate)
            if final_source_failure:
                raise ValueError("final source changed")
        finally:
            state["active"] = False

    source_module = SimpleNamespace(_git_source_paths=lambda root: ["Cargo.lock"],
                                    workspace_source_manifest=lambda root: "c" * 64,
                                    retained_workspace_source=source_owner)
    monkeypatch.setattr(candidate, "reviewed_helper", lambda root: source_module)
    monkeypatch.setattr(candidate.sys, "platform", "linux")
    monkeypatch.setattr(candidate.platform, "machine", lambda: "x86_64")
    monkeypatch.setattr(candidate.resource, "getrlimit", lambda limit: (4096, 4096))
    monkeypatch.setattr(candidate.subprocess, "check_output", lambda argv, **kw: (
        candidate.OBSERVER_COMMIT if argv[-1] == "HEAD" else ""))

    def command(argv, work, env, logs, label, **kwargs):
        assert state["active"]
        stdout = logs / (label + ".stdout")
        raw = b""
        if label == "rustc-version":
            raw = b"rustc 1.99.1 (fixture)\nhost: x86_64-unknown-linux-gnu\n"
        elif label == "publisher-build":
            publisher = output / "bootstrap" / candidate.NATIVE_TARGET / "release/iroha_release_bundle"
            publisher.parent.mkdir(parents=True)
            publisher.write_bytes(b"publisher; never executed by this unit control")
            publisher.chmod(0o700)
        elif label == "bridge-publication":
            assert argv[1] == "build-bridge-publish" and kwargs["pass_fds"]
            invocation = argv[argv.index("--name") + 1]
            bridge = output / "artifacts/sumeragi-release" / ("c" * 64) / "bridges" / invocation
            bridge.parent.mkdir(parents=True)
            staging, pins = fixture(bridge.parent)
            staging.rename(bridge)
            provenance = json.loads((bridge / "publisher-source.json").read_bytes())
            provenance.update(cargo_lock_sha256=candidate.digest(source / "Cargo.lock"),
                              build_environment_policy_sha256=candidate.digest(policy_path),
                              cargo_tool={"sha256": candidate.digest(cargo)},
                              rustc_tool={"sha256": candidate.digest(rustc)})
            (bridge / "publisher-source.json").write_text(json.dumps(provenance))
            raw = (f"bridge_dir\t{bridge}\nmodule_sha256\t{pins['module_sha256']}\n"
                   f"provenance_sha256\t{candidate.digest(bridge / 'publisher-source.json')}\n").encode()
        stdout.write_bytes(raw)
        return stdout

    monkeypatch.setattr(candidate, "run_logged", command)
    args = SimpleNamespace(source_root=str(source), output_root=str(output), cargo_home=str(cargo_home),
                           cargo_bin=str(cargo), rustc_bin=str(rustc), driver_revision="a" * 40)
    if final_source_failure:
        with pytest.raises(ValueError, match="final source changed"):
            candidate.prepare(args)
        assert not (output / "review/candidate.json").exists()
    else:
        result = candidate.prepare(args)
        assert result["status"] == "requires_independent_approval"
        assert result["bridge_executed"] is False
        assert (output / "review/candidate.json").is_file()
    assert not state["active"] and state["validated"] >= 3


@pytest.mark.parametrize("mutation", [None, "add-link", "remove-link", "content", "permissions", "replace-name"])
def test_publisher_accepts_existing_build_hardlinks_but_refuses_later_changes(tmp_path, mutation):
    root = tmp_path.resolve()
    publisher, alias = root / "publisher", root / "deps-alias"
    publisher.write_bytes(b"original executable")
    publisher.chmod(0o700)
    os.link(publisher, alias)

    def exercise():
        with candidate.held_publisher(custody, publisher) as (_, descriptor, observed):
            assert os.fstat(descriptor).st_nlink == 2
            assert observed == hashlib.sha256(b"original executable").hexdigest()
            if mutation == "add-link":
                os.link(publisher, root / "new-alias")
            elif mutation == "remove-link":
                alias.unlink()
            elif mutation == "content":
                alias.write_bytes(b"changed via other name")
            elif mutation == "permissions":
                alias.chmod(0o777)
            elif mutation == "replace-name":
                publisher.unlink()
                publisher.write_bytes(b"original executable")
                publisher.chmod(0o700)
    if mutation is None:
        exercise()
    else:
        with pytest.raises(ValueError, match="changed"):
            exercise()


@pytest.mark.parametrize("shape", ["symlink", "fifo", "writable", "set-id", "not-executable"])
def test_publisher_rejects_indirect_or_unsafe_initial_build_input(tmp_path, shape):
    root = tmp_path.resolve()
    publisher = root / "publisher"
    if shape == "symlink":
        (root / "actual").write_bytes(b"executable")
        publisher.symlink_to(root / "actual")
    elif shape == "fifo":
        os.mkfifo(publisher)
    else:
        publisher.write_bytes(b"executable")
        publisher.chmod({"writable": 0o777, "set-id": 0o4700, "not-executable": 0o600}[shape])
    with pytest.raises(ValueError, match="owner-controlled executable"):
        with candidate.held_publisher(custody, publisher):
            raise AssertionError("unsafe input was admitted")


def test_publisher_rejects_replaced_ancestor_even_when_file_identity_matches(tmp_path):
    root = tmp_path.resolve()
    parent = root / "build"
    parent.mkdir()
    publisher = parent / "publisher"
    publisher.write_bytes(b"executable")
    publisher.chmod(0o700)
    with pytest.raises(custody.SourceCustodyError, match="selected name changed"):
        with candidate.held_publisher(custody, publisher):
            parent.rename(root / "original-build")
            parent.mkdir()
            (root / "original-build/publisher").rename(publisher)
