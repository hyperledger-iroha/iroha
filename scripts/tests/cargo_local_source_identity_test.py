"""Local source preflight integrity and real Cargo freshness regression tests."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import cargo_local_source_identity as local


def stamp(root: Path, digest: str = "ab" * 32, paths=()) -> str:
    return (f"{local.STAMP_HEADER}\nroot={os.fsencode(root).hex()}\n"
            f"digest={digest}\nwatches={local.watch_digest(paths)}\n")


def test_stamp_preserves_unchanged_inode_and_mtime_and_rejects_links(tmp_path):
    directory = local.private_directory(tmp_path / "target")
    path = directory / "snapshot-v1"
    contents = stamp(tmp_path)
    local.update_stamp(path, contents)
    before = path.stat()
    local.update_stamp(path, contents)
    after = path.stat()
    assert (before.st_ino, before.st_mtime_ns) == (after.st_ino, after.st_mtime_ns)
    assert local.checked_stamp(path, tmp_path, "ab" * 32, ())["digest"] == "ab" * 32
    with pytest.raises(ValueError, match="differs"):
        local.checked_stamp(path, tmp_path, "cd" * 32, ())
    path.unlink()
    victim = tmp_path / "unrelated"
    victim.write_text("preserve me")
    path.symlink_to(victim)
    with pytest.raises(ValueError, match="unsafe"):
        local.update_stamp(path, "replacement")
    assert victim.read_text() == "preserve me"
    path.unlink()
    os.link(victim, path)
    with pytest.raises(ValueError, match="unsafe"):
        local.update_stamp(path, "replacement")
    assert victim.read_text() == "preserve me"


@pytest.mark.parametrize("change", ["duplicate", "unknown", "short", "no-newline", "bad-hex"])
def test_records_reject_malformed_fields(tmp_path, change):
    contents = stamp(tmp_path)
    if change == "duplicate":
        contents += "digest=" + "ab" * 32 + "\n"
    elif change == "unknown":
        contents += "extra=ab\n"
    elif change == "short":
        contents = contents.replace("ab" * 32, "ab")
    elif change == "no-newline":
        contents = contents.rstrip("\n")
    else:
        contents = contents.replace("digest=ab", "digest=AB")
    with pytest.raises(ValueError):
        local.parse_record(contents, local.STAMP_HEADER, local.STAMP_KEYS)


def test_private_directory_rejects_permissive_or_symlinked_state(tmp_path):
    output = tmp_path / "output"
    output.mkdir()
    directory = output / ".iroha-local-source"
    directory.mkdir(mode=0o755)
    with pytest.raises(ValueError, match="unsafe"):
        local.private_directory(output)
    directory.rmdir()
    other = tmp_path / "other"
    other.mkdir(mode=0o700)
    directory.symlink_to(other, target_is_directory=True)
    with pytest.raises(ValueError, match="unsafe"):
        local.private_directory(output)


def test_selected_graph_skips_unrelated_packages_but_keeps_uncertain_selections():
    packages = [
        {"name": "tiny", "id": "tiny-id", "dependencies": []},
        {"name": "native", "id": "native-id", "dependencies": [{"name": "iroha_plonk"}]},
        {"name": "iroha_plonk", "id": "plonk-id", "dependencies": []},
    ]
    metadata = {"packages": packages, "workspace_default_members": ["tiny-id"]}
    assert not local.may_use_plonk(metadata, ["check", "-p", "tiny"], None)
    assert not local.may_use_plonk(metadata, ["check"], None)
    assert local.may_use_plonk(metadata, ["check", "--package=native"], None)
    assert local.may_use_plonk(metadata, ["check", "--workspace"], None)
    assert local.may_use_plonk(metadata, ["check", "-p", "native@0.1"], None)
    assert local.may_use_plonk({}, ["check"], None)


def test_cargo_driver_retains_inherited_jobserver_descriptors(tmp_path, monkeypatch):
    read_fd, write_fd = os.pipe()
    try:
        os.set_inheritable(read_fd, True)
        monkeypatch.setenv("VERGEN_GIT_SHA", "local-fast-build")
        monkeypatch.delenv("IROHA_GIT_COMMIT_HASH", raising=False)
        monkeypatch.setattr(local, "output_roots", lambda *_: [])
        result = local.run(tmp_path, sys.executable, ["-c", f"import os; os.fstat({read_fd})"])
        assert result == 0
    finally:
        os.close(read_fd)
        os.close(write_fd)


def test_phase_timings_preserve_cargo_status_and_skipped_preflight(tmp_path, monkeypatch):
    monkeypatch.setenv("VERGEN_GIT_SHA", "local-fast-build")
    monkeypatch.delenv("IROHA_GIT_COMMIT_HASH", raising=False)
    monkeypatch.setattr(local, "output_roots", lambda *_: [])
    timings = {}
    assert local.run(tmp_path, sys.executable, ["-c", "raise SystemExit(7)"], timings) == 7
    assert set(timings) == {"discovery_seconds", "cargo_seconds"}
    assert all(value >= 0 for value in timings.values())


def test_phase_records_refusal_without_suppressing_it(monkeypatch):
    ticks = iter((2.0, 3.25, 5.0, 7.0))
    monkeypatch.setattr(local.time, "perf_counter", lambda: next(ticks))
    timings = {}
    with pytest.raises(ValueError, match="refused"):
        with local.phase(timings, "preflight_after_seconds"):
            raise ValueError("refused")
    with local.phase(timings, "preflight_after_seconds"):
        pass
    assert timings == {"preflight_after_seconds": 3.25}


@pytest.mark.parametrize("stderr", ["native source changed during identity computation\n", ""])
def test_failed_evaluator_preserves_stderr_in_refusal(tmp_path, monkeypatch, stderr):
    executable = tmp_path / "build-script-build"
    executable.write_text("unused evaluator fixture")
    record = {"executable": os.fsencode(executable).hex()}
    probe = (tmp_path / "receipt", record)
    monkeypatch.setattr(local, "checked_probe", lambda *_: record)
    failure = subprocess.CalledProcessError(101, [str(executable)], stderr=stderr)

    def fail(*_args, **_kwargs):
        raise failure

    monkeypatch.setattr(local.subprocess, "run", fail)
    with pytest.raises(ValueError, match="native source evaluator exited 101") as refused:
        local.evaluate(probe, tmp_path, [tmp_path])
    assert refused.value.__cause__ is failure
    if stderr:
        assert stderr.strip() in str(refused.value)


@pytest.mark.parametrize("arguments,expected", [
    (["check", "-p", "swarm", "--lib"], False),
    (["build", "-p", "swarm", "--bin", "swarm"], False),
    (["run", "-p", "swarm", "--", "--example"], False),
    (["clippy", "-p", "swarm", "--lib", "--", "-D", "warnings"], False),
    (["clippy", "-p", "swarm", "--fix"], True),
    (["test", "-p", "swarm", "--lib"], True),
    (["bench", "-p", "swarm"], True),
    (["check", "-p", "swarm", "--all-targets"], True),
    (["check", "-p", "swarm", "--examples"], True),
    (["check", "-p", "swarm", "--example=example"], True),
    (["check", "-p", "swarm", "--tests"], True),
    (["check", "-p", "swarm", "--test", "test"], True),
    (["build", "-p", "swarm", "--benches"], True),
    (["build", "-p", "swarm", "--bench=bench"], True),
    (["rustc", "-p", "swarm", "--lib"], True),
    (["alias", "-p", "swarm"], True),
    (["+nightly", "check", "-p", "swarm", "--lib"], True),
    (["--offline", "check", "-p", "swarm", "--lib"], True),
])
def test_normal_graph_skips_dev_only_plonk_but_other_targets_keep_it(arguments, expected):
    packages = [
        {"name": "swarm", "id": "swarm-id", "dependencies": [
            {"name": "model", "kind": None}, {"name": "native", "kind": "dev"}]},
        {"name": "model", "id": "model-id", "dependencies": []},
        {"name": "native", "id": "native-id", "dependencies": [
            {"name": "iroha_plonk", "kind": None}]},
        {"name": "iroha_plonk", "id": "plonk-id", "dependencies": []},
    ]
    metadata = {"packages": packages, "workspace_default_members": ["swarm-id"]}
    assert local.uses_dev_dependencies(arguments) is expected
    assert local.may_use_plonk(metadata, arguments, None) is expected
    # Optional/target-specific normal edges still need the conservative census.
    packages[0]["dependencies"][1].update(kind=None, optional=True, target="cfg(unix)")
    assert local.may_use_plonk(metadata, arguments, None)


@pytest.fixture(scope="module")
def real_workspace(tmp_path_factory):
    if not shutil.which("cargo") or not shutil.which("git"):
        pytest.skip("real Cargo/Git are required")
    root = tmp_path_factory.mktemp("native-source-cargo").resolve()
    (root / "crates/iroha_plonk/build_support").mkdir(parents=True)
    (root / "crates/iroha_plonk/src/keys/keygen/builtin").mkdir(parents=True)
    (root / "consumer/src").mkdir(parents=True)
    (root / "scripts").mkdir()
    for path in ("crates/iroha_plonk/build.rs", "crates/iroha_plonk/build_support/source_identity.rs",
                 "scripts/cargo_local_source_identity.py", "scripts/check_cargo_target_owner.py"):
        shutil.copy2(ROOT / path, root / path)
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers=["crates/iroha_plonk", "consumer"]\nresolver="2"\n'
        '[profile.dev]\ndebug=0\n')
    (root / "crates/iroha_plonk/Cargo.toml").write_text(
        '[package]\nname="iroha_plonk"\nversion="0.0.0"\nedition="2024"\n'
        '[build-dependencies]\nsha2="=0.10.9"\n')
    (root / "crates/iroha_plonk/src/lib.rs").write_text(
        '//! Tiny consumer of the actual maintained build script.\n'
        'mod identity { include!(concat!(env!("OUT_DIR"), "/native_source_identity.rs")); }\n'
        'pub fn identity() -> [u8; 32] { identity::CURRENT_NATIVE_SOURCE_DIGEST }\n')
    (root / "crates/iroha_plonk/src/keys/keygen/builtin/generated.rs").write_text("// registry DATA\n")
    (root / "consumer/Cargo.toml").write_text(
        '[package]\nname="consumer"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\niroha_plonk={path="../crates/iroha_plonk"}\n')
    (root / "consumer/src/lib.rs").write_text(
        '//! Downstream freshness witness.\npub fn identity() -> [u8; 32] { iroha_plonk::identity() }\n')
    (root / ".gitignore").write_text("/target/\n**/ignored_*\n")
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(("CARGO_", "RUSTC_", "CMAKE_", "VERGEN_", "IROHA_GIT_"))}
    environment.update(CARGO_TARGET_DIR=str(root / "target"), RUSTC_WRAPPER="",
                       VERGEN_GIT_SHA="local-fast-build", CARGO_INCREMENTAL="1")
    subprocess.run(["git", "init", "--quiet", str(root)], check=True, capture_output=True)
    subprocess.run(["git", "-C", str(root), "add", "."], check=True, capture_output=True)
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=root, env=environment,
                   check=True, capture_output=True, text=True)
    return root, environment


def cargo(workspace, *, direct=False, environment=None, check=True):
    root, defaults = workspace
    selected = defaults.copy()
    if direct:
        for key in ("VERGEN_GIT_SHA", "IROHA_GIT_COMMIT_HASH", local.CONTEXT):
            selected.pop(key, None)
    selected.update(environment or {})
    arguments = ["check", "--locked", "--offline", "-p", "consumer", "--message-format=json"]
    prefix = ["cargo"] if direct else [sys.executable, str(root / "scripts/cargo_local_source_identity.py"),
                                      "--source-root", str(root), "--"]
    result = subprocess.run([*prefix, *arguments], cwd=root, env=selected,
                            capture_output=True, text=True)
    if check:
        assert result.returncode == 0, result.stderr
    artifacts = {}
    for line in result.stdout.splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if record.get("reason") == "compiler-artifact" and record["target"]["kind"] == ["lib"]:
            artifacts[record["target"]["name"]] = record["fresh"]
    return result, artifacts


def test_real_cargo_noop_and_complete_membership_invalidation(real_workspace):
    root, environment = real_workspace
    cargo(real_workspace)  # Conservative first build registers the exact host evaluator.
    cargo(real_workspace)  # The local context becomes part of Cargo's stable fingerprint.
    _, artifacts = cargo(real_workspace)
    assert artifacts["iroha_plonk"] and artifacts["consumer"]
    probe = local.find_probe(root, [root / "target"])
    assert probe is not None
    output = probe[0].parent
    assert {path.name for path in output.iterdir()} == {"native_source_identity.rs"}
    stamp_path = root / "target/.iroha-local-source/snapshot-v1"
    before_stamp = stamp_path.stat()
    cargo(real_workspace)
    assert stamp_path.stat().st_mtime_ns == before_stamp.st_mtime_ns

    # Index-only staging and a same-byte unconsumed asset touch change no native
    # identity. They must not force the mathematical crate/consumer dirty.
    unconsumed = root / "crates/iroha_plonk/build_support/parameters.bin"
    unconsumed.write_bytes(b"fixed parameters")
    cargo(real_workspace)
    stable_stamp = stamp_path.stat().st_mtime_ns
    os.utime(unconsumed, None)
    subprocess.run(["git", "-C", str(root), "add", "."], check=True, capture_output=True)
    _, unchanged = cargo(real_workspace)
    assert unchanged["iroha_plonk"] and unchanged["consumer"]
    assert stamp_path.stat().st_mtime_ns == stable_stamp

    for relative in ("new-native-owner/asset.bin", "crates/iroha_plonk/src/ignored_asset.bin",
                     "crates/iroha_plonk/build_support/ignored_generator.rs", ".cargo/config.toml"):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        for contents in ("# first\n", "# second\n"):
            previous = stamp_path.read_text()
            path.write_text(contents)
            _, artifacts = cargo(real_workspace)
            assert not artifacts["iroha_plonk"] and not artifacts["consumer"], relative
            assert stamp_path.read_text() != previous
            _, unchanged = cargo(real_workspace)
            assert unchanged["iroha_plonk"] and unchanged["consumer"], relative
        previous = stamp_path.read_text()
        path.unlink()
        _, artifacts = cargo(real_workspace)
        assert not artifacts["iroha_plonk"] and not artifacts["consumer"], relative
        assert stamp_path.read_text() != previous

    # A source link escaping the census must fail before Cargo can borrow a cache.
    escaped = root / "crates/iroha_plonk/src/ignored_escape"
    escaped.symlink_to("/etc/hosts")
    result, artifacts = cargo(real_workspace, check=False)
    assert result.returncode == 3 and not artifacts
    escaped.unlink()
    cargo(real_workspace)

    # A caller-supplied context cannot override the wrapper's full maintained scan.
    added = root / "untracked-native-input"
    added.write_text("new member\n")
    _, artifacts = cargo(real_workspace, environment={local.CONTEXT: str(stamp_path)})
    assert not artifacts["iroha_plonk"]
    added.unlink()
    cargo(real_workspace)

    # Ordinary Cargo deliberately preserves the old conservative rescan behavior.
    cargo(real_workspace, direct=True)
    _, artifacts = cargo(real_workspace, direct=True)
    assert not artifacts["iroha_plonk"] and not artifacts["consumer"]
    # Local coordination cannot alter the exact direct/release output contract.
    assert {path.name for path in output.iterdir()} == {"native_source_identity.rs"}
    assert local.RECEIPT_HEADER not in (output / "native_source_identity.rs").read_text()
    assert local.find_probe(root, [root / "target"]) is None
    direct_output = (output.parent / "output").read_text().splitlines()
    assert {line.partition("=")[2] for line in direct_output
            if line.startswith("cargo:rerun-if-env-changed=")} == {
        "CARGO_CFG_TARGET_ARCH", "CARGO_CFG_TARGET_OS", "CARGO_CFG_TARGET_POINTER_WIDTH",
        "CARGO_CFG_TARGET_ENDIAN", "TARGET",
    }
    cargo(real_workspace)
    cargo(real_workspace)


def test_real_build_script_rejects_forged_or_linked_context(real_workspace):
    root, _ = real_workspace
    probe = local.find_probe(root, [root / "target"])
    assert probe is not None
    executable = local.record_path(probe[1]["executable"])
    out = probe[0].parent
    bad = root / "target/.iroha-local-source/snapshot-v1"
    local.update_stamp(bad, stamp(root, "00" * 32))
    environment = dict(os.environ, OUT_DIR=str(out), CARGO_MANIFEST_DIR=str(root / "crates/iroha_plonk"),
                       VERGEN_GIT_SHA="local-fast-build", **{local.CONTEXT: str(bad)})
    environment.pop("IROHA_GIT_COMMIT_HASH", None)
    result = subprocess.run([str(executable)], env=environment, capture_output=True, text=True)
    assert result.returncode != 0 and "authoritative build-script census" in result.stderr
    bad.unlink()
    other = bad.parent / "other"
    other.write_text(stamp(root))
    bad.symlink_to(other)
    result = subprocess.run([str(executable)], env=environment, capture_output=True, text=True)
    assert result.returncode != 0 and "unsafe local source" in result.stderr
    bad.unlink()


def test_probe_rejects_changed_executable_or_compiled_sources(real_workspace):
    root, _ = real_workspace
    probe = local.find_probe(root, [root / "target"])
    assert probe is not None
    path, _ = probe
    original = path.read_text()
    path.write_text(original.replace("executable_sha256=", "executable_sha256=00"))
    assert local.find_probe(root, [root / "target"]) is None
    path.write_text(original)
    source = root / "crates/iroha_plonk/build.rs"
    old = source.read_text()
    source.write_text(old + "\n// changed evaluator\n")
    assert local.find_probe(root, [root / "target"]) is None
    result, artifacts = cargo(real_workspace)
    assert "no matching evaluator" in result.stderr
    assert not artifacts["iroha_plonk"]
    assert local.find_probe(root, [root / "target"]) is not None
    cargo(real_workspace)
    _, artifacts = cargo(real_workspace)
    assert artifacts["iroha_plonk"] and artifacts["consumer"]
    source.write_text(old)


def test_source_drift_after_cargo_is_rejected(real_workspace):
    root, environment = real_workspace
    cargo(real_workspace)
    cargo(real_workspace)
    driver = root / "post-cargo-driver"
    driver.write_text(
        f'#!{sys.executable}\nimport pathlib, subprocess, sys\n'
        'result = subprocess.call(["cargo", *sys.argv[1:]])\n'
        'pathlib.Path("post-cargo-native-input").write_text("new input after compilation")\n'
        'raise SystemExit(result)\n')
    driver.chmod(0o700)
    result = subprocess.run(
        [sys.executable, str(root / "scripts/cargo_local_source_identity.py"),
         "--source-root", str(root), "--cargo-driver", str(driver), "--", "check",
         "--locked", "--offline", "-p", "consumer"],
        cwd=root, env=environment, capture_output=True, text=True,
    )
    assert result.returncode == 3 and "inputs changed during Cargo" in result.stderr
    (root / "post-cargo-native-input").unlink()
    driver.unlink()
