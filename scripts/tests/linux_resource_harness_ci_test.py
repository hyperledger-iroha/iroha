"""Exercise the CI invoker's source/build/receipt chain without claiming native qualification."""

from __future__ import annotations

from contextlib import contextmanager
import copy
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from types import SimpleNamespace
import unittest

import pytest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("linux_harness_ci", ROOT / "ci/run_linux_resource_harness.py")
CI = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CI)
NATIVE = "x86_64-unknown-linux-gnu"
PACKAGE = "path+file:///source/crates/iroha_release_bundle#2.0.0"
PIN = "a" * 64


def metadata(repository):
    """Construct the documented Cargo metadata fields for the selected package."""
    package = repository / "crates/iroha_release_bundle"
    return {"version": 1, "packages": [{
        "name": "iroha_release_bundle", "id": PACKAGE, "source": None,
        "manifest_path": str(package / "Cargo.toml"), "targets": [{
            "name": "iroha_release_bundle", "kind": ["bin"], "crate_types": ["bin"],
            "src_path": str(package / "src/bin/iroha_release_bundle.rs"),
            "required-features": ["dev-tools"],
        }],
    }]}


def events(repository, target):
    """Construct Cargo protocol records, not an authenticated executable fixture."""
    package = repository / "crates/iroha_release_bundle"
    executable = target / NATIVE / "debug/iroha_release_bundle"
    return [{
        "reason": "compiler-artifact", "package_id": PACKAGE,
        "manifest_path": str(package / "Cargo.toml"),
        "target": {"name": "iroha_release_bundle", "kind": ["bin"], "crate_types": ["bin"],
                   "src_path": str(package / "src/bin/iroha_release_bundle.rs")},
        "features": ["dev-tools"], "profile": {"test": False}, "fresh": False,
        "executable": str(executable), "filenames": [str(executable)],
    }, {"reason": "build-finished", "success": True}]


def encoded_events(items):
    """Encode the Cargo line protocol exactly as the controller consumes it."""
    return b"".join(json.dumps(item).encode() + b"\n" for item in items)


@pytest.mark.parametrize("mutation", ["none", "foreign-manifest", "foreign-source", "registry",
                                      "duplicate-package", "duplicate-target", "feature", "missing-id"])
def test_metadata_binds_one_reviewed_publisher(tmp_path, mutation):
    value = metadata(tmp_path)
    package = value["packages"][0]
    if mutation == "foreign-manifest":
        package["manifest_path"] += ".other"
    elif mutation == "foreign-source":
        package["targets"][0]["src_path"] += ".other"
    elif mutation == "registry":
        package["source"] = "registry+https://example.invalid"
    elif mutation == "duplicate-package":
        value["packages"].append(copy.deepcopy(package))
    elif mutation == "duplicate-target":
        package["targets"].append(copy.deepcopy(package["targets"][0]))
    elif mutation == "feature":
        package["targets"][0]["required-features"] = []
    elif mutation == "missing-id":
        del package["id"]
    if mutation == "none":
        assert CI.publisher_package(json.dumps(value).encode(), tmp_path) == PACKAGE
    else:
        with pytest.raises(ValueError):
            CI.publisher_package(json.dumps(value).encode(), tmp_path)


@pytest.mark.parametrize("mutation", ["none", "foreign-package", "foreign-manifest", "foreign-source",
    "foreign-executable", "wrong-kind", "wrong-features", "cached", "test-profile", "missing-filename",
    "duplicate-artifact", "missing-terminal", "failure", "post-terminal", "unknown-event"])
def test_cargo_protocol_requires_fresh_exact_success(tmp_path, mutation):
    value = events(tmp_path, tmp_path / "target")
    event = value[0]
    if mutation == "foreign-package":
        event["package_id"] = "foreign"
    elif mutation == "foreign-manifest":
        event["manifest_path"] += ".other"
    elif mutation == "foreign-source":
        event["target"]["src_path"] += ".other"
    elif mutation == "foreign-executable":
        event["executable"] += ".other"
    elif mutation == "wrong-kind":
        event["target"]["kind"] = ["lib"]
    elif mutation == "wrong-features":
        event["features"] = []
    elif mutation == "cached":
        event["fresh"] = True
    elif mutation == "test-profile":
        event["profile"]["test"] = True
    elif mutation == "missing-filename":
        event["filenames"] = []
    elif mutation == "duplicate-artifact":
        value.insert(0, copy.deepcopy(event))
    elif mutation == "missing-terminal":
        value.pop()
    elif mutation == "failure":
        value[-1]["success"] = False
    elif mutation == "post-terminal":
        value.append({"reason": "compiler-message"})
    elif mutation == "unknown-event":
        value.insert(0, {"reason": "unknown"})
    if mutation == "none":
        assert CI.publisher_from_events(encoded_events(value), tmp_path, tmp_path / "target", NATIVE,
                                        PACKAGE) == tmp_path / "target" / NATIVE / "debug/iroha_release_bundle"
    else:
        with pytest.raises(ValueError):
            CI.publisher_from_events(encoded_events(value), tmp_path, tmp_path / "target", NATIVE, PACKAGE)


@pytest.mark.parametrize("mutation", ["none", "foreign-path", "uppercase", "missing-lf", "extra-field", "duplicate-field", "crlf"])
def test_only_the_original_complete_receipt_supplies_pins(tmp_path, mutation):
    raw = f"bridge_dir\t{tmp_path}\nmodule_sha256\t{PIN}\nprovenance_sha256\t{'b' * 64}\n".encode()
    if mutation == "foreign-path":
        raw = raw.replace(str(tmp_path).encode(), b"/foreign")
    elif mutation == "uppercase":
        raw = raw.replace(PIN.encode(), PIN.upper().encode())
    elif mutation == "missing-lf":
        raw = raw[:-1]
    elif mutation == "extra-field":
        raw += b"additional\tvalue\n"
    elif mutation == "duplicate-field":
        raw = raw.replace(b"provenance_sha256", b"module_sha256")
    elif mutation == "crlf":
        raw = raw.replace(b"\n", b"\r\n")
    if mutation == "none":
        assert CI.bridge_receipt(raw, tmp_path)["module_sha256"] == PIN
    else:
        with pytest.raises(ValueError):
            CI.bridge_receipt(raw, tmp_path)


@pytest.mark.parametrize("mutation", ["none", "symlink", "replacement", "in-place"])
def test_retained_executable_rejects_aliases_and_changed_originals(tmp_path, mutation):
    root = tmp_path.resolve()
    path = root / "publisher"
    path.write_bytes(b"original")
    path.chmod(0o700)
    if mutation == "symlink":
        alias = root / "alias"
        alias.symlink_to(path)
        with pytest.raises(ValueError):
            CI.executable(alias)
        return
    def exercise():
        """Hold the original while a deterministic custody mutation occurs."""
        with CI.retained_executable(path) as (descriptor, revalidate):
            revalidate()
            assert os.read(descriptor, 8) == b"original"
            if mutation == "replacement":
                other = root / "replacement"
                other.write_bytes(b"original")
                other.chmod(0o700)
                other.replace(path)
            elif mutation == "in-place":
                path.write_bytes(b"changed!")
    if mutation == "none":
        exercise()
    else:
        with pytest.raises(ValueError, match="changed during use"):
            exercise()


@pytest.mark.parametrize("failure", ["none", "seal", "metadata", "build", "publication", "receipt", "source",
                                      "cargo-after-build", "rustc-after-publication", "tests"])
def test_controller_preserves_live_source_owner_and_original_receipt(tmp_path, monkeypatch, failure):
    """Fault-inject process boundaries; no fixture is admitted by a real loader here."""
    root = tmp_path.resolve()
    repository = root / "source"
    (repository / "scripts").mkdir(parents=True)
    (repository / "scripts/environment-policy.json").write_text(json.dumps({"allowed": ["PATH"]}))
    cargo_home = root / "cargo-home"
    cargo_home.mkdir()
    for name in ("cargo", "rustc"):
        (root / name).write_bytes(b"fixture tool")
        (root / name).chmod(0o700)
    args = SimpleNamespace(source_root=repository, source_commit="c" * 40,
                           cargo_bin=root / "cargo", rustc_bin=root / "rustc", cargo_home=cargo_home,
                           run_root=root / "invocation")
    calls = []
    owner = SimpleNamespace(live=False)
    @contextmanager
    def retained(source, manifest):
        """Expose explicit ownership ordering to the child-process assertions."""
        assert source == repository and manifest == PIN
        owner.live = True
        try:
            yield owner
        finally:
            owner.live = False
    def revalidate():
        """Reject source mutation before publication pins reach a consumer."""
        assert owner.live
        if failure == "source" and "publication" in calls:
            raise ValueError("source changed")
    owner.revalidate = revalidate
    source_module = SimpleNamespace(native_artifact_workspace_source_manifest=lambda source: PIN,
                                    retained_workspace_source=retained)
    real_spec = CI.importlib.util.spec_from_file_location
    def selected_spec(name, path):
        """Substitute only the explicit source-owner boundary for this unit test."""
        if name == "ci_source_owner":
            return SimpleNamespace(name=name, loader=SimpleNamespace(exec_module=lambda value: None))
        return real_spec(name, path)
    monkeypatch.setattr(CI.importlib.util, "spec_from_file_location", selected_spec)
    monkeypatch.setattr(CI.importlib.util, "module_from_spec", lambda spec: source_module)
    monkeypatch.setattr(CI.sys, "platform", "linux")
    monkeypatch.setenv("RUSTFLAGS", "--cfg unwanted_inheritance")
    def child(command, **kwargs):
        """Model exact protocol outputs and assert each ordering/closed-environment fence."""
        if "--validate-source-commit" in command:
            calls.append("seal")
            assert not owner.live and command[-1] == args.source_commit
            if failure == "seal":
                raise subprocess.CalledProcessError(1, command)
            return subprocess.CompletedProcess(command, 0)
        assert owner.live
        assert "RUSTFLAGS" not in kwargs["env"]
        if "metadata" in command:
            phase = "metadata"
            value = metadata(repository)
            if failure == phase:
                value["packages"][0]["manifest_path"] = "/foreign"
            kwargs["stdout"].write(json.dumps(value).encode())
        elif "build" in command:
            phase = "build"
            assert "--locked" in command and "--offline" in command
            assert "--no-default-features" in command
            target = args.run_root / "bootstrap"
            binary = target / NATIVE / "debug/iroha_release_bundle"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"fixture output")
            binary.chmod(0o700)
            kwargs["stdout"].write(encoded_events(events(repository, target)))
            if failure == "cargo-after-build":
                args.cargo_bin.write_bytes(b"changed original")
        elif "build-bridge-publish" in command:
            phase = "publication"
            assert command[0] == f"/proc/self/fd/{kwargs['pass_fds'][0]}"
            assert os.fstat(kwargs["pass_fds"][0]).st_size == len(b"fixture output")
            bridge = args.run_root / "artifacts/sumeragi-release" / PIN / "bridges/invocation.ci0001"
            # Deliberately no selected module exists: only original receipt pins
            # can feed the next command; hashing disk output would fail this test.
            raw = f"bridge_dir\t{bridge}\nmodule_sha256\t{PIN}\nprovenance_sha256\t{'b' * 64}\n"
            kwargs["stdout"].write(raw.encode() if failure != "receipt" else b"partial\n")
            if failure == "rustc-after-publication":
                args.rustc_bin.write_bytes(b"changed original")
        else:
            phase = "tests"
            assert command[-1] == "--all-tests"
            assert command[command.index("--module-sha256") + 1] == PIN
            assert command[command.index("--provenance-sha256") + 1] == "b" * 64
        calls.append(phase)
        return subprocess.CompletedProcess(command, 1 if failure == phase and phase != "metadata" else 0)
    monkeypatch.setattr(CI.subprocess, "run", child)
    monkeypatch.setattr(CI.subprocess, "check_output", lambda command, **kwargs:
                        b"" if command[0] == "git" else f"host: {NATIVE}\n")
    if failure == "none":
        CI.run(args)
        assert calls == ["seal", "metadata", "build", "publication", "tests"]
        assert (args.run_root / "evidence/completed.json").is_file()
        assert (args.run_root.stat().st_mode & 0o777) == 0o700
    else:
        with pytest.raises((ValueError, subprocess.CalledProcessError)):
            CI.run(args)
        assert not (args.run_root / "evidence/completed.json").exists()
        if failure != "tests":
            assert "tests" not in calls
    assert not owner.live


def test_native_runner_is_importable_with_isolated_python():
    """The explicit script must locate both trusted sibling directories under -I."""
    result = subprocess.run([sys.executable, "-I", "-S", str(ROOT / "scripts/tests/run_zk_resource_harness_native.py"),
                             "--help"], text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    assert "--all-tests" in result.stdout


def test_ci_requires_normal_publication_and_all_tests_without_changing_memfd_policy():
    """The mandatory job owns both the native suite and its lightweight regressions."""
    workflow = (ROOT / ".github/workflows/pr.yml").read_text()
    guards = workflow.split("  rust_source_guards:\n", 1)[1].split("\n  rust_affected:", 1)[0]
    assert re.search(r"(?m)^    (?:needs|if):", guards) is None
    assert "scripts/tests/zk_resource_harness_test.py" not in guards
    assert "scripts/tests/linux_resource_harness_ci_test.py" in guards
    setup = guards.split("      - name: Provision the normal Linux observer prerequisites\n", 1)[1]
    setup, native = setup.split("      - name: Run the complete harness through the normal native publisher\n", 1)
    native = native.split("      - name: Retain Linux harness publication evidence\n", 1)[0]
    assert "if:" not in setup and "if:" not in native
    assert "sudo sysctl -w kernel.perf_event_paranoid=2" in setup
    assert 'test "$(sysctl -n kernel.perf_event_paranoid)" = 2' in setup
    assert 'RUSTC="$rustc_bin" "$cargo_bin" fetch --locked --target "$native_target"' in setup
    assert "memfd_noexec" not in guards and "setcap" not in guards
    assert "ulimit -n 65536" in native
    assert "python3 -I -S -B ci/run_linux_resource_harness.py" in native
    for argument in ("source-root", "source-commit", "cargo-bin", "rustc-bin", "cargo-home", "run-root"):
        assert "--" + argument in native
    assert "continue-on-error" not in native and "|| true" not in native
    assert "SOURCE_GUARDS_RESULT: ${{ needs.rust_source_guards.result }}" in workflow


@pytest.mark.parametrize("failure", ["none", "skip", "missing", "failed"])
def test_all_tests_runner_keeps_the_whole_module_and_refuses_partial_results(monkeypatch, failure):
    """Inspect the actual suite selected by --all-tests and mutate completion receipts."""
    spec = importlib.util.spec_from_file_location("harness_runner", ROOT / "scripts/tests/run_zk_resource_harness_native.py")
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    expected = unittest.defaultTestLoader.loadTestsFromModule(runner.cases).countTestCases()
    assert expected >= 81
    native = unittest.defaultTestLoader.loadTestsFromTestCase(runner.cases.NativeRunTests).countTestCases()
    assert native == 20 and expected > native
    original_fixture = runner.cases.NativeRunTests.publication_fixture
    monkeypatch.setattr(runner.cases.NativeRunTests, "publication_fixture", original_fixture)
    monkeypatch.setattr(runner.sys, "platform", "linux")
    monkeypatch.setattr(runner.sys, "argv", ["runner", "--source-root", "/source", "--bridge-root", "/bridge",
        "--module-sha256", PIN, "--provenance-sha256", PIN, "--source-manifest-sha256", PIN,
        "--native-target", NATIVE, "--all-tests"])
    def execute(suite):
        """Verify actual loader coverage without impersonating a native test pass."""
        assert suite.countTestCases() == expected
        return SimpleNamespace(wasSuccessful=lambda: failure != "failed",
                               testsRun=expected - (failure == "missing"),
                               skipped=[("control", "unexpected")] if failure == "skip" else [])
    monkeypatch.setattr(runner.unittest, "TextTestRunner", lambda **kwargs: SimpleNamespace(run=execute))
    assert runner.main() == (0 if failure == "none" else 1)
