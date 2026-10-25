"""Local profiler preserves warm targets and reports actual Cargo/child evidence."""

import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
from types import SimpleNamespace

import pytest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import profile_cargo_local as local
import profile_cargo_build as api
import check_cargo_target_owner as owner


def test_cargo_arguments_keep_native_parallelism_and_compile_only_tests():
    arguments = local.cargo_arguments(["--", "test", "-p", "owner", "--lib"], None)
    assert arguments == ["test", "-p", "owner", "--lib", "--locked", "--offline",
                         "--message-format=json", "--timings", "--no-run"]
    assert local.cargo_arguments(["check"], 4)[-2:] == ["--jobs", "4"]


@pytest.mark.parametrize("arguments", [["run"], ["check", "--config=x"],
    ["check", "--target-dir", "elsewhere"], ["test", "--", "filter"],
    ["check", "--message-format=json"], ["check", "-j2"], ["check", "--help"],
    ["build", "-h"], ["test", "--list"], ["check", "--version"],
    ["check", "--build-dir=elsewhere"]])
def test_cargo_arguments_reject_ambiguous_measurement_controls(arguments):
    with pytest.raises(ValueError):
        local.cargo_arguments(arguments, 3)


def test_phase_times_and_summary_keep_failed_or_rebuilt_samples():
    phases = local.phase_times('ordinary stderr\n[cargo-fast-timings] '
        '{"cargo_seconds": 2, "discovery_seconds": 1, "preflight_before_seconds": 3, '
        '"preflight_after_seconds": 4, "lock_seconds": 7}\n')
    assert phases["source_preflight_seconds"] == 8
    samples = [{**phases, "elapsed_seconds": value, "returncode": code, "compiled": compiled}
               for value, code, compiled in ((20, 0, 0), (30, 1, 2))]
    result = local.summarize(samples)
    assert result["elapsed_seconds"] == {"median": 25, "worst": 30}
    assert not result["all_successful"] and not result["zero_rebuilt_artifacts"]
    assert local.summarize([]) == {}


@pytest.mark.parametrize("record", ["", '[cargo-fast-timings] {}',
    '[cargo-fast-timings] {"cargo_seconds": -1}',
    '[cargo-fast-timings] {"cargo_seconds": true}',
    '[cargo-fast-timings] {"cargo_seconds": NaN}',
    '[cargo-fast-timings] {"cargo_seconds": 1}\n[cargo-fast-timings] {"cargo_seconds": 2}'])
def test_phase_times_reject_missing_or_invalid_evidence(record):
    with pytest.raises(ValueError):
        local.phase_times(record)


def test_private_instrument_reuse_refuses_links_and_content_changes(tmp_path):
    directory = local.private_directory(tmp_path / "state")
    path = directory / "instrument"
    local.pinned_file(path, b"original", 0o400)
    before = path.stat()
    local.pinned_file(path, b"original", 0o400)
    assert path.stat().st_mtime_ns == before.st_mtime_ns
    with pytest.raises(ValueError, match="changed"):
        local.pinned_file(path, b"changed", 0o400)
    link = directory / "link"
    link.symlink_to(path)
    with pytest.raises(ValueError, match="changed"):
        local.pinned_file(link, b"original", 0o400)
    with pytest.raises(ValueError, match="unsafe"):
        local.private_directory(link)
    size = local.target_size(directory)
    assert size["files"] == 1 and size["bytes"] == len(b"original")


@pytest.fixture
def measured_run(tmp_path, monkeypatch):
    """Exercise report admission and real source/target checks without compiling."""
    root = (tmp_path / "workspace").resolve()
    root.mkdir()
    (root / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
    (root / "lib.rs").write_text("//! Unchanged fixture source.\n")
    toolchain = tmp_path / "toolchain"
    toolchain.mkdir()
    compiler, cargo = toolchain / "rustc", toolchain / "cargo"
    compiler.write_bytes(b"pinned rustc bytes")
    cargo.write_bytes(b"pinned cargo bytes")
    state = SimpleNamespace(root=root, target=tmp_path / "target", out=tmp_path / "report.json",
                            compiler=compiler, cargo=cargo, calls=[], on_cargo=lambda: None)
    state.artifact = {
        "reason": "compiler-artifact", "package_id": "fixture@0.1.0",
        "manifest_path": str(root / "Cargo.toml"), "fresh": True, "features": [],
        "target": {"name": "fixture", "src_path": str(root / "lib.rs"),
                   "kind": ["lib"], "crate_types": ["lib"]},
        "profile": {"opt_level": "0", "debuginfo": 0, "debug_assertions": True,
                    "overflow_checks": True, "test": False},
    }
    state.messages = [state.artifact, {"reason": "build-finished", "success": True}]
    state.phases = {"cargo_seconds": 1, "discovery_seconds": 0.1,
                    "preflight_before_seconds": 0.2, "preflight_after_seconds": 0.2}
    for key in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        monkeypatch.delenv(key, raising=False)
    monkeypatch.setattr(local, "host_load", lambda: {"load_average": [0, 0, 0], "compiler_processes": []})
    monkeypatch.setattr(local.platform, "platform", lambda: "fixture-host")
    monkeypatch.setattr(local, "native_snapshot", lambda *_: {"required": False})
    monkeypatch.setattr(api, "tracked_and_untracked_paths", lambda *_: ["lib.rs"])
    monkeypatch.setattr(api, "git_index_entries", lambda *_: ())
    monkeypatch.setattr(api, "validate_git_worktree", lambda *_: None)
    monkeypatch.setattr(api, "closed_git_revision", lambda *_: "fixture-revision")

    def check_output(command, **kwargs):
        if command[:2] == ["rustup", "which"]:
            return str(toolchain / command[2]) + "\n"
        assert command in ([str(compiler), "-vV"], [str(cargo), "-vV"])
        return Path(command[0]).name + " fixture-version\n"

    def run(command, **kwargs):
        state.calls.append((command, kwargs["env"].copy()))
        if Path(command[1]).name == "check_cargo_target_owner.py":
            owner.check_lane_role(Path(kwargs["env"]["CARGO_TARGET_DIR"]), root)
        else:
            assert Path(command[0]).name == "cargo_fast.sh"
            state.on_cargo()
            for message in state.messages:
                kwargs["stdout"].write(json.dumps(message) + "\n")
            kwargs["stderr"].write(local.TIMING_PREFIX + json.dumps(state.phases) + "\n")
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(local.subprocess, "check_output", check_output)
    monkeypatch.setattr(local.subprocess, "run", run)

    def invoke():
        result = local.run(["--root", str(root), "--target-dir", str(state.target),
                            "--out", str(state.out), "--", "check", "--lib"], api)
        return result, json.loads(state.out.read_text())

    state.invoke = invoke
    return state


def test_rejected_release_target_has_no_profiler_state_writes(measured_run):
    case = measured_run
    marker = case.target / ".taira-build-lane/role.json"
    marker.parent.mkdir(parents=True)
    marker.write_text(json.dumps({"schema": "taira.cargo-lane.v1", "role": "release",
                                  "repo_root": str(case.root)}))
    marker.chmod(0o600)
    before = marker.read_bytes()
    result, report = case.invoke()
    assert result == 3 and not report["valid"]
    assert "authenticated release lane" in report["error"]
    assert not (case.target / ".iroha-local-profile").exists()
    assert {path.name for path in case.target.iterdir()} == {".taira-build-lane"}
    assert marker.read_bytes() == before
    assert len(case.calls) == 1


@pytest.mark.parametrize("kind", ["empty", "unknown", "incomplete", "failed", "duplicate-finish"])
def test_invalid_cargo_evidence_cannot_claim_a_fresh_success(measured_run, kind):
    case = measured_run
    if kind == "empty":
        case.messages = [{"reason": "build-finished", "success": True}]
    elif kind == "unknown":
        case.messages[0] = {"reason": "unknown-artifact", "fresh": True}
    elif kind == "incomplete":
        case.messages[0] = {"reason": "compiler-artifact", "fresh": True}
    elif kind == "failed":
        case.messages[1]["success"] = False
    else:
        case.messages.append({"reason": "build-finished", "success": True})
    result, report = case.invoke()
    assert result == 3 and not report["valid"]
    assert "artifact" in report["error"] or "successful build" in report["error"]
    assert report["samples"][0]["returncode"] == 0
    assert not report.get("summary", {}).get("zero_rebuilt_artifacts", False)


@pytest.mark.parametrize("phases", [{}, {"discovery_seconds": 0.1, "preflight_before_seconds": 0.2}])
def test_failed_preflight_keeps_sample_and_log_without_cargo_timing(measured_run, monkeypatch, phases):
    case = measured_run
    case.messages = []
    case.phases = phases
    run = local.subprocess.run

    def fail_preflight(command, **kwargs):
        completed = run(command, **kwargs)
        if Path(command[0]).name == "cargo_fast.sh":
            kwargs["stderr"].write("cargo-fast: native source preflight failed: source changed\n")
            completed.returncode = 3
        return completed

    monkeypatch.setattr(local.subprocess, "run", fail_preflight)
    result, report = case.invoke()
    assert result == 3 and not report["valid"]
    assert len(report["samples"]) == 1
    sample = report["samples"][0]
    assert sample["returncode"] == 3 and sample["elapsed_seconds"] >= 0
    assert sample["fresh"] == sample["compiled"] == 0
    assert report["error"] == f"local command exited 3; see {sample['stderr']}"
    assert "source changed" in Path(sample["stderr"]).read_text()
    assert not report.get("summary", {}).get("zero_rebuilt_artifacts", False)
    assert len(case.calls) == 2


def test_missing_native_evaluator_refuses_measurement(tmp_path, monkeypatch):
    monkeypatch.setattr(local.native, "output_roots", lambda *_: [tmp_path / "target"])
    monkeypatch.setattr(local.native, "find_probe", lambda *_: None)
    monkeypatch.setattr(local.native, "evaluate", lambda *_: pytest.fail("missing evaluator was executed"))
    with pytest.raises(ValueError, match="native evaluator is not ready"):
        local.native_snapshot(tmp_path, ["check"], {})


def test_native_snapshot_uses_the_complete_maintained_record(tmp_path, monkeypatch):
    probe = (tmp_path / "receipt", {"executable_sha256": "ab" * 32})
    outputs = [tmp_path / "target"]
    monkeypatch.setattr(local.native, "output_roots", lambda *_: outputs)
    monkeypatch.setattr(local.native, "find_probe", lambda *_: probe)
    calls = []

    def evaluate(selected, root, selected_outputs):
        calls.append((selected, root, selected_outputs))
        return (f"{local.native.STAMP_HEADER}\nroot={os.fsencode(root).hex()}\n"
                f"digest={'ab' * 32}\nwatches={'cd' * 32}\n")

    monkeypatch.setattr(local.native, "evaluate", evaluate)
    snapshot = local.native_snapshot(tmp_path, ["check"], {})
    assert snapshot == {"required": True, "root": os.fsencode(tmp_path).hex(),
                        "digest": "ab" * 32, "watches": "cd" * 32}
    assert calls == [(probe, tmp_path, outputs)]


@pytest.mark.parametrize("change", ["source", "native", "toolchain", "missing-native-phases"])
def test_source_native_or_toolchain_drift_invalidates_saved_report(measured_run, monkeypatch, change):
    case = measured_run
    if change == "source":
        case.on_cargo = lambda: (case.root / "lib.rs").write_text("//! Changed source.\n")
        error = "local source changed"
    elif change == "toolchain":
        case.on_cargo = lambda: case.compiler.write_bytes(b"changed compiler")
        error = "toolchain changed"
    else:
        snapshots = iter([{"required": True, "digest": "ab" * 32},
                          {"required": True, "digest": "cd" * 32}])
        monkeypatch.setattr(local, "native_snapshot", lambda *_: next(snapshots))
        error = "native census changed"
        if change == "missing-native-phases":
            case.phases = {"cargo_seconds": 1, "discovery_seconds": 0.1}
            error = "omitted complete pre/post census"
    result, report = case.invoke()
    assert result == 3 and not report["valid"]
    assert error in report["error"]
    assert report["schema"] == "iroha-local-build-diagnostic-v1"
    assert report["release_qualification"] is False
    assert len(report["samples"]) == 1
    if change == "source":
        assert report["source_unchanged"] is False


def test_command_and_report_use_pinned_toolchain_paths_and_bytes(measured_run, monkeypatch):
    case = measured_run
    monkeypatch.setenv("PATH", "/caller/custom/path:/usr/bin:/bin")
    result, report = case.invoke()
    assert result == 0 and report["valid"]
    for _, environment in case.calls:
        assert environment["RUSTC"] == str(case.compiler)
        assert environment["PATH"].split(os.pathsep)[0] == str(case.cargo.parent)
        assert environment["CARGO_TARGET_DIR"] == str(case.target)
        assert environment["RUSTC_WRAPPER"] == environment["RUSTC_WORKSPACE_WRAPPER"] == ""
    for name, path in (("cargo", case.cargo), ("rustc", case.compiler)):
        assert report["toolchain"][name]["path"] == str(path)
        assert report["toolchain"][name]["sha256"] == hashlib.sha256(path.read_bytes()).hexdigest()
    assert report["summary"]["zero_rebuilt_artifacts"]
    assert report["samples"][0]["fresh"] == 1


@pytest.mark.parametrize("key", ["RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"])
def test_custom_compiler_overrides_fail_before_target_writes(measured_run, monkeypatch, key):
    case = measured_run
    monkeypatch.setenv(key, "/caller/compiler-or-wrapper")
    result, report = case.invoke()
    assert result == 3 and not report["valid"]
    assert "unset custom compiler/wrapper" in report["error"]
    assert not case.target.exists()
    assert not case.calls


GITLINK_OID = "3f379811fcc86cca2345ec59c56686501b710afe"


@pytest.fixture
def gitlink_repo(tmp_path):
    """A real local index with an optional uninitialized sibling, without Cargo."""
    root = tmp_path / "repo"
    root.mkdir()
    environment = local.git_environment(os.environ)
    git = shutil.which("git")
    if git is None:
        pytest.skip("Git is required")
    subprocess.run([git, "init", "--quiet", str(root)], env=environment, check=True)
    (root / "source.rs").write_text("//! Retained source.\n")
    subprocess.run([git, "-C", str(root), "add", "source.rs"], env=environment, check=True)
    subprocess.run([git, "-C", str(root), "update-index", "--add", "--cacheinfo",
                    f"160000,{GITLINK_OID},iroha-docs"], env=environment, check=True)
    return root, environment, git


def test_local_gitlink_records_bind_oid_and_missing_or_empty_state(gitlink_repo, monkeypatch):
    root, environment, git = gitlink_repo
    records = []
    canonical = api.canonical_json_bytes
    def collect(value):
        assert set(value) == {"bytes", "executable", "kind", "path", "sha256"}
        records.append(value)
        return canonical(value)
    monkeypatch.setattr(api, "canonical_json_bytes", collect)
    missing = local.source_snapshot(root, environment, git, api)
    assert missing.files == 2 and missing.deleted == 0
    assert missing.bytes == (root / "source.rs").stat().st_size
    link_record = next(record for record in records if record["kind"] == "gitlink")
    assert link_record["bytes"] == 0 and link_record["executable"] is False
    assert link_record["path"] == "iroha-docs"
    assert hashlib.sha256(b"".join(canonical(record) + b"\n" for record in records)).hexdigest() == missing.sha256
    (root / "iroha-docs").mkdir()
    empty = local.source_snapshot(root, environment, git, api)
    assert empty != missing
    assert local.source_snapshot(root, environment, git, api) == empty
    subprocess.run([git, "-C", str(root), "update-index", "--cacheinfo",
                    f"160000,{'b' * 40},iroha-docs"], env=environment, check=True)
    assert local.source_snapshot(root, environment, git, api) != empty
    with pytest.raises(ValueError, match="submodules unsupported"):
        api.source_fingerprint(root, ["iroha-docs", "source.rs"])
    with pytest.raises(ValueError, match="submodules unsupported"):
        api.capture_source_snapshot(root, ["iroha-docs"], root.parent / "isolated")


@pytest.mark.parametrize("kind", ["populated", "initialized", "file", "symlink", "dangling"])
def test_local_gitlinks_reject_every_nonempty_or_link_type(gitlink_repo, kind):
    root, environment, git = gitlink_repo
    path = root / "iroha-docs"
    if kind in ("populated", "initialized"):
        path.mkdir()
        (path / (".git" if kind == "initialized" else "source.rs")).write_text("unread source")
    elif kind == "file":
        path.write_text("not a submodule directory")
    else:
        path.symlink_to(root / ("source.rs" if kind == "symlink" else "absent"))
    with pytest.raises(ValueError, match="local gitlink"):
        local.source_snapshot(root, environment, git, api)


@pytest.mark.parametrize("kind", ["file", "symlink"])
def test_gitlink_missing_ancestor_must_not_be_a_file_or_symlink(tmp_path, kind):
    if kind == "file":
        (tmp_path / "parent").write_text("blocked")
    else:
        (tmp_path / "parent").symlink_to(tmp_path / "absent")
    with pytest.raises(ValueError, match="local gitlink"):
        api.source_fingerprint(tmp_path, ["parent/docs"], gitlinks={"parent/docs": GITLINK_OID})


def test_gitlink_missing_ancestor_and_sha256_commit_are_supported(tmp_path):
    missing = api.source_fingerprint(tmp_path, ["parent/docs"], gitlinks={"parent/docs": "a" * 64})
    (tmp_path / "parent").mkdir()
    assert api.source_fingerprint(tmp_path, ["parent/docs"], gitlinks={"parent/docs": "a" * 64}) == missing
    (tmp_path / "parent/docs").mkdir()
    assert api.source_fingerprint(tmp_path, ["parent/docs"], gitlinks={"parent/docs": "a" * 64}) != missing


@pytest.mark.parametrize("change", ["populate", "swap", "appear", "appear-disappear", "ancestor-swap"])
def test_gitlink_witnesses_reject_changes_during_source_reads(tmp_path, monkeypatch, change):
    root = tmp_path / "repo"
    root.mkdir()
    relative = "parent/docs" if change == "ancestor-swap" else "docs"
    link = root / relative
    if change not in ("appear", "appear-disappear"):
        link.mkdir(parents=True)
    (root / "source.rs").write_text("source")
    read = api._read_regular_stable_at
    def mutate(*args):
        if change == "populate":
            (link / ".git").write_text("initialized")
        elif change == "swap":
            link.rename(tmp_path / "old")
            link.mkdir()
        elif change == "ancestor-swap":
            link.parent.rename(tmp_path / "old")
            link.mkdir(parents=True)
        else:
            link.mkdir()
            if change == "appear-disappear":
                link.rmdir()
        return read(*args)
    monkeypatch.setattr(api, "_read_regular_stable_at", mutate)
    with pytest.raises(ValueError, match="gitlink"):
        api.source_fingerprint(root, [relative, "source.rs"], gitlinks={relative: GITLINK_OID})


def test_gitlink_replacement_between_lstat_and_open_is_refused(tmp_path, monkeypatch):
    (tmp_path / "docs").mkdir()
    (tmp_path / "elsewhere").mkdir()
    opened = api._open_child_directory_at
    def swap(parent, name, before=None, **kwargs):
        if name == "docs":
            (tmp_path / "docs").rmdir()
            (tmp_path / "docs").symlink_to(tmp_path / "elsewhere")
        return opened(parent, name, before, **kwargs)
    monkeypatch.setattr(api, "_open_child_directory_at", swap)
    with pytest.raises((ValueError, OSError)):
        api.source_fingerprint(tmp_path, ["docs"], gitlinks={"docs": GITLINK_OID})


@pytest.mark.parametrize("mapping", [{"elsewhere": GITLINK_OID}, {"docs": "0" * 40},
                                      {"docs": "x" * 40}, {"docs": "a" * 39}])
def test_gitlink_mapping_must_be_selected_and_have_a_commit_identity(tmp_path, mapping):
    with pytest.raises(ValueError):
        api.source_fingerprint(tmp_path, ["docs"], gitlinks=mapping)


@pytest.mark.parametrize("raw", [b"160000 bad 0\tdocs\0", b"160000 " + b"a" * 40 + b" 4\tdocs\0",
    b"160000 " + b"a" * 40 + b" 0\t../docs\0", b"160000 " + b"a" * 40 + b" 0\t.\0",
    b"160000 " + b"a" * 40 + b" 0\tdocs", (b"160000 " + b"a" * 40 + b" 0\tdocs\0") * 2])
def test_git_index_parser_rejects_invalid_or_duplicate_records(tmp_path, monkeypatch, raw):
    monkeypatch.setattr(api.subprocess, "check_output", lambda *_args, **_kwargs: raw)
    with pytest.raises(ValueError):
        api.git_index_entries(tmp_path, {}, "/git", tmp_path)


def test_local_gitlink_conflict_is_not_silently_collapsed(gitlink_repo, monkeypatch):
    root, environment, git = gitlink_repo
    monkeypatch.setattr(api, "git_index_entries", lambda *_: (
        ("iroha-docs", "160000", GITLINK_OID, 1),
        ("iroha-docs", "100644", "a" * 40, 2),
    ))
    with pytest.raises(ValueError, match="conflicted local gitlink"):
        local.source_snapshot(root, environment, git, api)


@pytest.mark.parametrize("change", ["index", "membership", "gitlink-swap"])
def test_local_snapshot_rechecks_index_membership_and_gitlink_witness(gitlink_repo, monkeypatch, change):
    root, environment, git = gitlink_repo
    link = root / "iroha-docs"
    link.mkdir()
    calls = 0
    inventory = api.tracked_and_untracked_paths
    def mutate(*args):
        nonlocal calls
        calls += 1
        if calls == 2:
            if change == "index":
                subprocess.run([git, "-C", str(root), "update-index", "--cacheinfo",
                                f"160000,{'b' * 40},iroha-docs"], env=environment, check=True)
            elif change == "membership":
                (root / "new.rs").write_text("new member")
            else:
                link.rename(root.parent / "old-docs")
                link.mkdir()
        return inventory(*args)
    monkeypatch.setattr(api, "tracked_and_untracked_paths", mutate)
    with pytest.raises(ValueError, match="changed"):
        local.source_snapshot(root, environment, git, api)


def test_local_git_discovery_strips_inherited_overrides(gitlink_repo):
    root, environment, git = gitlink_repo
    expected = local.source_snapshot(root, environment, git, api)
    injected = dict(environment, GIT_DIR="/absent/git", GIT_WORK_TREE="/absent/worktree",
                    GIT_INDEX_FILE="/absent/index", GIT_CONFIG_COUNT="1",
                    GIT_CONFIG_KEY_0="core.worktree", GIT_CONFIG_VALUE_0="/absent/override")
    assert local.source_snapshot(root, injected, git, api) == expected
    assert injected["GIT_DIR"] == "/absent/git", "Cargo's original environment was mutated"


def test_local_snapshot_rejects_wrong_worktree(gitlink_repo):
    root, environment, git = gitlink_repo
    other = root.parent / "other"
    other.mkdir()
    subprocess.run([git, "-C", str(root), "config", "core.worktree", str(other)], env=environment, check=True)
    with pytest.raises(ValueError, match="worktree does not match"):
        local.source_snapshot(root, environment, git, api)


@pytest.mark.skipif(shutil.which("cargo") is None or shutil.which("rustup") is None,
                    reason="real Cargo and Rustup required")
def test_real_local_profile_reuses_compiler_and_target_and_refuses_report_overwrite(tmp_path, monkeypatch):
    root = tmp_path / "workspace"
    scripts = root / "scripts"
    scripts.mkdir(parents=True)
    for name in ("cargo_fast.sh", "cargo_local_source_identity.py", "check_cargo_target_owner.py"):
        shutil.copy2(ROOT / "scripts" / name, scripts / name)
    shutil.copy2(ROOT / "rust-toolchain.toml", root / "rust-toolchain.toml")
    (root / "Cargo.toml").write_text('[package]\nname="local_profile_fixture"\nversion="0.1.0"\nedition="2024"\n[lib]\npath="lib.rs"\n')
    (root / "lib.rs").write_text('//! Local profiling fixture.\npub fn value() -> u8 { 7 }\n')
    for key in ("IROHA_GIT_COMMIT_HASH", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        monkeypatch.delenv(key, raising=False)
    subprocess.run(["git", "init", "--quiet", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "add", "."], check=True)
    monkeypatch.setattr(api, "closed_git_revision", lambda *_: "fixture-uncommitted")
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=root, check=True)
    out = tmp_path / "reports" / "first.json"
    arguments = ["--local", "--root", str(root), "--target-dir", str(tmp_path / "target"),
                 "--out", str(out), "--measure-compiler", "--warmup", "1", "--repeat", "2", "--", "check", "--lib"]
    assert api.main(arguments) == 0
    report = json.loads(out.read_text())
    assert report["valid"] and report["source_unchanged"]
    assert report["summary"]["zero_rebuilt_artifacts"]
    assert report["warmup"][0]["compiled"] == 1
    assert len(report["warmup"][0]["compiler_measurements"]["compiled"]) == 1
    assert report["warmup"][0]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] > 0
    original = out.read_bytes()
    assert api.main(arguments) == 3
    assert out.read_bytes() == original
    arguments[arguments.index(str(out))] = str(out.with_name("second.json"))
    assert api.main(arguments) == 0
    second = json.loads(out.with_name("second.json").read_text())
    assert second["warmup"][0]["compiled"] == 0
    assert second["instrumentation"] == report["instrumentation"]
