"""Focused native carrier custody and truthful build observations; no Cargo/signing."""
from __future__ import annotations

import copy
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("native_carrier_test", ROOT / "scripts/taira_native_carrier_build.py")
owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner)


def tool(path):
    return {"path": path, "invocation": path, "sha256": "c" * 64, "size": 1}


@pytest.fixture
def plan():
    target = "/owned/warm-target"
    source = "/owned/signed-source"
    tools = {name: tool("/usr/bin/" + name) for name in owner.TOOL_NAMES}
    env = {"PATH": "/usr/bin:/bin", "HOME": "/owned", "CARGO_HOME": "/owned/cargo-home",
           "CARGO_TARGET_DIR": target, "CARGO": tools["cargo"]["path"], "RUSTC": tools["rustc"]["path"],
           "RUSTDOC": tools["rustdoc"]["path"], "RUSTUP_TOOLCHAIN": "1.93.1", "CARGO_BUILD_JOBS": "6",
           "CARGO_INCREMENTAL": "1", "CARGO_NET_OFFLINE": "true", "CARGO_PROFILE_DEV_SPLIT_DEBUGINFO": "unpacked",
           "CARGO_PROFILE_TEST_SPLIT_DEBUGINFO": "unpacked", "CARGO_ENCODED_RUSTFLAGS": "-Clinker=/usr/bin/compiler\x1f-Clink-arg=-fuse-ld=/usr/bin/linker",
           "IROHA_GIT_COMMIT_HASH": "a" * 40, "VERGEN_GIT_SHA": "a" * 40,
           "CARGO_ZIGBUILD_PYTHON_PATH": "/usr/bin/false", "CARGO_ZIGBUILD_ZIG_PATH": source + "/scripts/zig_linux_gnu.py",
           "CC_ENABLE_DEBUG_OUTPUT": "1", "LC_ALL": "C", "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1"}
    return {"schema": owner.PLAN_SCHEMA, "source": {"repo_root": source, "commit": "a" * 40, "tree": "b" * 40,
            "signer": "D" * 40, "cargo_lock_sha256": "e" * 64,
            "public_key": {"path": "/owned/signer-public.gpg", "sha256": "f" * 64, "size": 100}},
            "target_dir": target, "lane_owner_repo_root": "/owned/old-source", "output_dir": "/owned/attempt",
            "tools": tools, "environment": env, "capacity": {"cargo_additional_bytes": 0, "capture_additional_bytes": 0}}


def test_closed_dev_plan_accepts_zero_floor_and_separate_lane_owner(plan):
    assert owner.validate_plan(plan) is plan
    assert plan["lane_owner_repo_root"] != plan["source"]["repo_root"]
    assert plan["capacity"]["cargo_additional_bytes"] == 0


@pytest.mark.parametrize("mutate,reason", [
    (lambda p: p.update(native_release_qualified=True), "plan fields"),
    (lambda p: p["source"].update(signer="D" * 16), "fingerprint"),
    (lambda p: p["environment"].update(CARGO_BUILD_JOBS="1"), "fixed dev policy"),
    (lambda p: p["environment"].update(CARGO_NET_OFFLINE="false"), "fixed dev policy"),
    (lambda p: p["environment"].update(RUSTC_WRAPPER="/tmp/hook"), "environment fields"),
    (lambda p: p["environment"].update(RUSTFLAGS="-Canything"), "environment fields"),
    (lambda p: p["environment"].update(IROHA_GIT_COMMIT_HASH="b" * 40), "fixed dev policy"),
    (lambda p: p.update(output_dir="/owned/warm-target/evidence"), "separate"),
    (lambda p: p.update(target_dir="/owned/../warm-target"), "normalized"),
    (lambda p: p["capacity"].update(cargo_additional_bytes=True), "capacity"),
    (lambda p: p["tools"]["cargo"].update(sha256="C" * 64), "digest"),
    (lambda p: p["environment"].update(PATH="/usr/bin::/bin"), "public path"),
])
def test_plan_rejects_unpinned_or_qualification_inputs(plan, mutate, reason):
    mutate(plan)
    with pytest.raises(RuntimeError, match=reason):
        owner.validate_plan(plan)


def test_read_plan_pins_original_canonical_owner_bytes(tmp_path, plan):
    path = tmp_path / "plan.json"
    raw = owner.canonical(plan)
    path.write_bytes(raw); path.chmod(0o600)
    actual, retained, info = owner.read_plan(path)
    assert actual == plan and retained == raw and info.sha256 == owner.sha(raw)
    path.write_text(json.dumps(plan, indent=2))
    with pytest.raises(RuntimeError, match="canonical"):
        owner.read_plan(path)
    path.write_bytes(raw); path.chmod(0o644)
    with pytest.raises(RuntimeError, match="owner-only"):
        owner.read_plan(path)


def test_exact_command_has_only_two_native_offline_dev_carriers():
    cmd = owner.build_command(Path("/frozen"), Path("/warm"), "/pinned/cargo")
    assert cmd == ["/pinned/cargo", "build", "--config", "/frozen/.cargo/config.toml", "--manifest-path", "/frozen/Cargo.toml",
                   "--target-dir", "/warm", "--locked", "--offline", "--message-format=json-render-diagnostics",
                   "-p", "irohad", "--bin", "iroha3d_taira", "-p", "iroha_cli", "--bin", "iroha"]
    assert not any(value in cmd for value in ("test", "clean", "zigbuild", "--target", "release"))


def elf(machine=183, kind=3):
    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", header, 16, kind, machine, 1)
    return bytes(header)


@pytest.mark.parametrize("header", [b"", elf(62), elf(kind=1), b"\x7fELF\x01" + b"\0" * 59])
def test_elf_rejects_other_cpu_or_non_executable(header):
    with pytest.raises(RuntimeError, match="AArch64"):
        owner.elf_header(header)


def test_elf_accepts_native_executable():
    owner.elf_header(elf())
    owner.elf_header(elf(kind=2))


def expected_graph(source=Path("/frozen")):
    owners = {}
    for name, package in owner.BINARIES:
        base = source / "crates" / package / "bins"
        owners[name] = {"package": package, "package_id": "path+" + base.as_uri() + "#" + package + "@2.0.0-rc.2.0",
                        "manifest_path": str(base / "Cargo.toml"), "required_features": ["cli" if name == "iroha" else "daemon"],
                        "target": {"name": name, "kind": ["bin"], "crate_types": ["bin"],
                                   "src_path": str(base / "src/bin" / (name + ".rs")), "edition": "2024"}}
    return {"schema": "taira.native-carrier-build.cargo-owners.v1", "owners": owners}


def emission(name, package, source=Path("/frozen"), target=Path("/warm")):
    binding = expected_graph(source)["owners"][name]
    executable = str(target / "debug" / name)
    return {"reason": "compiler-artifact", "target": {**binding["target"], "required-features": binding["required_features"]},
            "package_id": binding["package_id"], "profile": {"test": False},
            "manifest_path": binding["manifest_path"], "executable": executable,
            "filenames": [executable], "fresh": True}


def write_emissions(path, rows):
    path.write_bytes(b"".join((json.dumps(row, separators=(",", ":")) + "\n").encode() for row in rows))


@pytest.fixture
def cargo_graph(tmp_path):
    source, target = tmp_path / "frozen", tmp_path / "warm"
    source.mkdir(); target.mkdir()
    signed = []

    def retain(relative, raw):
        path = source / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            path.chmod(0o600)
        path.write_bytes(raw); path.chmod(0o400)
        row = {"path": relative, "kind": "regular", "index_mode": "100644", "mode": 0o400,
               "sha256": owner.sha(raw), "size": len(raw), "object": "a" * 40}
        signed[:] = [old for old in signed if old["path"] != relative] + [row]
        return path

    retain("Cargo.toml", b'[workspace.package]\nversion="2.0.0-rc.2.0"\nedition="2024"\n')
    packages = []
    for name, package in owner.BINARIES:
        relative = "crates/" + package + "/bins"
        feature = "cli" if name == "iroha" else "daemon"
        manifest = retain(relative + "/Cargo.toml", (
            '[package]\nname="' + package + '"\nversion.workspace=true\nedition.workspace=true\n'
            '[[bin]]\nname="' + name + '"\npath="src/bin/' + name + '.rs"\n'
            'required-features=["' + feature + '"]\n').encode())
        src = retain(relative + "/src/bin/" + name + ".rs", b"fn main() {}\n")
        packages.append({"name": package, "version": "2.0.0-rc.2.0", "source": None,
                         "id": "path+" + manifest.parent.as_uri() + "#" + package + "@2.0.0-rc.2.0",
                         "manifest_path": str(manifest), "targets": [{"name": name, "kind": ["bin"],
                             "crate_types": ["bin"], "src_path": str(src), "edition": "2024",
                             "required-features": [feature]}]})
    metadata = {"version": 1, "workspace_root": str(source), "target_directory": str(target),
                "packages": packages, "workspace_members": [row["id"] for row in packages]}
    log = tmp_path / "cargo-metadata.stdout"

    def derive():
        log.write_bytes(owner.canonical(metadata)); log.chmod(0o400)
        return owner.carrier_owner_graph(log, source, target, signed)

    return SimpleNamespace(source=source, target=target, signed=signed, metadata=metadata,
                           log=log, derive=derive, retain=retain)


def test_actual_graph_binds_moved_bin_owners_signed_source_and_emissions(cargo_graph, tmp_path):
    graph = cargo_graph.derive()
    assert graph["metadata_sha256"] == owner.sha(cargo_graph.log.read_bytes())
    rows = []
    for name, package in owner.BINARIES:
        binding = graph["owners"][name]
        assert binding["manifest_path"].endswith("/" + package + "/bins/Cargo.toml")
        assert binding["manifest_sha256"] == owner.sha(Path(binding["manifest_path"]).read_bytes())
        assert binding["src_sha256"] == owner.sha(Path(binding["target"]["src_path"]).read_bytes())
        rows.append(emission(name, package, cargo_graph.source, cargo_graph.target))
    log = tmp_path / "build.stdout"
    write_emissions(log, [library_emission()] + rows + [{"reason": "build-finished", "success": True}])
    assert set(owner.artifact_emissions(log, cargo_graph.source, cargo_graph.target, graph)) == {"iroha", "iroha3d_taira"}


@pytest.mark.parametrize("mutate", [
    lambda g: g.metadata.update(workspace_root="/foreign"),
    lambda g: g.metadata.update(target_directory="/foreign"),
    lambda g: g.metadata.update(version=2),
    lambda g: g.metadata["packages"][0].update(source="registry+https://index.crates.io"),
    lambda g: g.metadata["packages"].append(copy.deepcopy(g.metadata["packages"][0])),
    lambda g: g.metadata["workspace_members"].pop(0),
    lambda g: g.metadata["workspace_members"].append(g.metadata["workspace_members"][0]),
    lambda g: g.metadata["packages"][0].update(id="path+file:///foreign#irohad@2.0.0-rc.2.0"),
    lambda g: g.metadata["packages"][0].update(manifest_path="/foreign/Cargo.toml"),
    lambda g: g.metadata["packages"][0].update(version="9.9.9"),
    lambda g: g.metadata["packages"][0]["targets"][0].update(kind=["lib"]),
    lambda g: g.metadata["packages"][0]["targets"][0].update(crate_types=["lib"]),
    lambda g: g.metadata["packages"][0]["targets"][0].update(src_path="/foreign/main.rs"),
    lambda g: g.metadata["packages"][0]["targets"][0].update(edition="2021"),
    lambda g: g.metadata["packages"][0]["targets"][0].update(**{"required-features": []}),
    lambda g: g.metadata["packages"][0]["targets"].append(copy.deepcopy(g.metadata["packages"][0]["targets"][0])),
    lambda g: g.signed.pop(0),
    lambda g: g.signed.append(copy.deepcopy(g.signed[0])),
])
def test_graph_rejects_unowned_or_ambiguous_native_metadata(cargo_graph, mutate):
    mutate(cargo_graph)
    with pytest.raises(RuntimeError, match="owner graph|signed source"):
        cargo_graph.derive()


@pytest.mark.parametrize("relative", ["Cargo.toml", "crates/irohad/bins/Cargo.toml",
                                      "crates/irohad/bins/src/bin/iroha3d_taira.rs"])
def test_graph_rejects_source_body_changes_even_when_native_metadata_is_unchanged(cargo_graph, relative):
    path = cargo_graph.source / relative
    path.chmod(0o600); path.write_bytes(path.read_bytes() + b"\n"); path.chmod(0o400)
    with pytest.raises(RuntimeError, match="signed source bytes differ"):
        cargo_graph.derive()


@pytest.mark.parametrize("replace", [
    ('name="irohad"', 'name="other_owner"'),
    ('name="iroha3d_taira"', 'name="other_bin"'),
    ('path="src/bin/iroha3d_taira.rs"', 'path="../../../../../outside.rs"'),
    ('version.workspace=true', 'version="9.9.9"'),
])
def test_graph_checks_signed_manifest_declarations_independently(cargo_graph, replace):
    relative = "crates/irohad/bins/Cargo.toml"
    raw = (cargo_graph.source / relative).read_bytes().decode().replace(*replace)
    cargo_graph.retain(relative, raw.encode())
    with pytest.raises(RuntimeError, match="owner graph"):
        cargo_graph.derive()


def test_graph_uses_declared_layout_instead_of_any_package_directory_allowlist(cargo_graph):
    row = cargo_graph.metadata["packages"][0]
    old = Path(row["manifest_path"])
    manifest = cargo_graph.retain("commands/server/owners/Cargo.toml", old.read_bytes())
    src = cargo_graph.retain("commands/server/owners/src/bin/iroha3d_taira.rs", b"fn main() {}\n")
    row.update(manifest_path=str(manifest), id="path+" + manifest.parent.as_uri() + "#irohad@2.0.0-rc.2.0")
    row["targets"][0]["src_path"] = str(src)
    cargo_graph.metadata["workspace_members"][0] = row["id"]
    assert cargo_graph.derive()["owners"]["iroha3d_taira"]["manifest_path"] == str(manifest)


def test_graph_binds_cargos_compact_local_package_id_to_the_signed_declared_owner(cargo_graph):
    row = cargo_graph.metadata["packages"][0]
    old = Path(row["manifest_path"])
    manifest = cargo_graph.retain("commands/irohad/Cargo.toml", old.read_bytes())
    src = cargo_graph.retain("commands/irohad/src/bin/iroha3d_taira.rs", b"fn main() {}\n")
    row.update(manifest_path=str(manifest), id="path+" + manifest.parent.as_uri() + "#2.0.0-rc.2.0")
    row["targets"][0]["src_path"] = str(src)
    cargo_graph.metadata["workspace_members"][0] = row["id"]
    assert cargo_graph.derive()["owners"]["iroha3d_taira"]["package_id"] == row["id"]


def test_emissions_require_authenticated_graph_without_old_path_fallback(tmp_path):
    with pytest.raises(RuntimeError, match="source-bound native Cargo owner graph"):
        owner.artifact_emissions(tmp_path / "absent", Path("/frozen"), Path("/warm"))


def test_success_requires_actual_both_cargo_emissions(tmp_path):
    log = tmp_path / "stdout"
    rows = [emission(name, package) for name, package in owner.BINARIES]
    write_emissions(log, rows + [{"reason": "build-finished", "success": True}])
    assert set(owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())) == {"iroha", "iroha3d_taira"}
    write_emissions(log, rows[:1] + [{"reason": "build-finished", "success": True}])
    with pytest.raises(RuntimeError, match="both"):
        owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())
    rows[0]["manifest_path"] = "/foreign/Cargo.toml"
    write_emissions(log, rows + [{"reason": "build-finished", "success": True}])
    with pytest.raises(RuntimeError, match="selected signed source"):
        owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())


def library_emission():
    return {"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["lib"]},
            "profile": {"test": False}, "manifest_path": "/frozen/crates/iroha/Cargo.toml",
            "executable": None, "filenames": ["/warm/debug/deps/libiroha.rlib", "/warm/debug/deps/libiroha.rmeta"],
            "fresh": False}


def test_iroha_library_name_collision_precedes_genuine_cli_and_daemon_bins(tmp_path):
    log = tmp_path / "stdout"
    cli, daemon = emission("iroha", "iroha_cli"), emission("iroha3d_taira", "irohad")
    cli["fresh"] = daemon["fresh"] = False
    write_emissions(log, [library_emission(), cli, daemon, {"reason": "build-finished", "success": True}])
    assert owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph()) == {"iroha": cli, "iroha3d_taira": daemon}


@pytest.mark.parametrize("invalid", [
    {"manifest_path": "/frozen/crates/iroha/Cargo.toml"},
    {"package_id": "path+file:///foreign#iroha_cli@2.0.0-rc.2.0"},
    {"target": {**expected_graph()["owners"]["iroha"]["target"], "src_path": "/foreign/iroha.rs"}},
    {"target": {**expected_graph()["owners"]["iroha"]["target"], "crate_types": ["lib"]}},
    {"profile": {"test": True}},
    {"executable": "/foreign/iroha"},
    {"filenames": []},
    {"filenames": None},
    {"profile": None},
])
def test_library_collision_does_not_admit_invalid_cli_bin(tmp_path, invalid):
    log = tmp_path / "stdout"
    cli = emission("iroha", "iroha_cli")
    cli.update(invalid)
    write_emissions(log, [library_emission(), cli, emission("iroha3d_taira", "irohad"),
                          {"reason": "build-finished", "success": True}])
    with pytest.raises(RuntimeError, match="selected signed source"):
        owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())


def test_library_collision_does_not_admit_duplicate_cli_bin(tmp_path):
    log = tmp_path / "stdout"
    cli = emission("iroha", "iroha_cli")
    write_emissions(log, [library_emission(), cli, emission("iroha3d_taira", "irohad"), cli,
                          {"reason": "build-finished", "success": True}])
    with pytest.raises(RuntimeError, match="selected signed source"):
        owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())


@pytest.mark.parametrize("finished", [[], [{"reason": "build-finished", "success": False}],
    [{"reason": "build-finished", "success": True}, {"reason": "build-finished", "success": True}]])
def test_library_collision_still_requires_one_successful_build_finished(tmp_path, finished):
    log = tmp_path / "stdout"
    write_emissions(log, [library_emission(), emission("iroha", "iroha_cli"), emission("iroha3d_taira", "irohad")] + finished)
    with pytest.raises(RuntimeError, match="both"):
        owner.artifact_emissions(log, Path("/frozen"), Path("/warm"), expected_graph())


def test_run_build_retains_exact_streams_failure_exit_and_inherited_locks(tmp_path, monkeypatch):
    seen = {}
    class Child:
        pid = 123
        def __init__(self, argv, **kwargs):
            seen.update(argv=argv, **kwargs)
            os.write(kwargs["stdout"], b"actual stdout\0\xff")
            os.write(kwargs["stderr"], b"actual compiler failure\r\n")
        def wait(self, timeout):
            assert timeout == 30
            return 17
    monkeypatch.setattr(owner.subprocess, "Popen", Child)
    cmd, env = ["/pinned/cargo", "build"], {"CARGO_BUILD_JOBS": "6"}
    assert owner.run_build(cmd, env, tmp_path, (11, 12, 13)) == 17
    assert (tmp_path / "build.stdout").read_bytes() == b"actual stdout\0\xff"
    assert (tmp_path / "build.stderr").read_bytes() == b"actual compiler failure\r\n"
    assert seen["cwd"] == "/" and seen["env"] == env and seen["pass_fds"] == (11, 12, 13)
    assert seen["start_new_session"] is True
    started = json.loads((tmp_path / "started.json").read_bytes())
    assert started["pid"] == 123 and started["argv"] == cmd
    assert not (tmp_path / "manifest.json").exists()
    with pytest.raises(FileExistsError):
        owner.run_build(cmd, env, tmp_path, (11,))


def test_interrupted_observer_never_signals_or_fabricates_exit(tmp_path, monkeypatch):
    class Child:
        pid = 456
        def __init__(self, argv, **kwargs):
            os.write(kwargs["stdout"], b"running original output")
        def wait(self, timeout):
            raise KeyboardInterrupt
        def terminate(self):
            pytest.fail("Cargo must never be terminated")
        def kill(self):
            pytest.fail("Cargo must never be killed")
    monkeypatch.setattr(owner.subprocess, "Popen", Child)
    with pytest.raises(KeyboardInterrupt):
        owner.run_build(["/pinned/cargo", "build"], {}, tmp_path, ())
    assert (tmp_path / "build.stdout").read_bytes() == b"running original output"
    assert not (tmp_path / "cargo-exit.json").exists() and not (tmp_path / "manifest.json").exists()


def test_native_platform_is_rejected_before_any_output(plan, monkeypatch):
    monkeypatch.setattr(owner.sys, "platform", "darwin")
    with pytest.raises(RuntimeError, match="native Linux ARM"):
        owner.execute(plan, owner.canonical(plan))


def test_cargo_home_rejects_global_config_and_credentials(tmp_path, plan, monkeypatch):
    plan["environment"]["CARGO_HOME"] = str(tmp_path)
    tmp_path.chmod(0o700)
    (tmp_path / "credentials.toml").write_text("not opened")
    with pytest.raises(RuntimeError, match="unexpected"):
        owner.check_cargo_home(plan)
    (tmp_path / "credentials.toml").unlink()
    monkeypatch.setattr(owner.os.path, "lexists", lambda p: str(p) == "/.cargo/config.toml")
    with pytest.raises(RuntimeError, match="root Cargo"):
        owner.check_cargo_home(plan)


def test_probe_retains_original_stderr_on_nonzero(tmp_path, monkeypatch):
    def child(command, env, output, lock_fds, *, label):
        (output / (label + ".stdout")).write_bytes(b"out")
        (output / (label + ".stderr")).write_bytes(b"exact\xfferror")
        return 9
    monkeypatch.setattr(owner, "run_build", child)
    with pytest.raises(RuntimeError, match="original output retained"):
        owner.run_probe(["/pinned/tool"], {}, tmp_path, "probe")
    assert (tmp_path / "probe.stderr").read_bytes() == b"exact\xfferror"


def test_tool_bytes_and_resolution_are_independently_pinned(tmp_path, plan, monkeypatch):
    paths = {}
    for name in owner.TOOL_NAMES:
        path = tmp_path / name
        raw = ("owned harmless " + name).encode()
        path.write_bytes(raw); path.chmod(0o500)
        paths[name] = path
        plan["tools"][name] = {"path": str(path), "invocation": str(path), "sha256": owner.sha(raw), "size": len(raw)}
    plan["environment"]["PATH"] = str(tmp_path)
    monkeypatch.setattr(owner.sys, "executable", str(paths["python"]))
    first = owner.verify_tools(plan)
    assert first["cargo"]["sha256"] == plan["tools"]["cargo"]["sha256"]
    paths["cargo"].chmod(0o700); paths["cargo"].write_bytes(b"changed tool bytes")
    with pytest.raises(RuntimeError, match="tool bytes differ"):
        owner.verify_tools(plan)


def test_public_key_classification_refuses_secret_before_python_reads_body(tmp_path, plan, monkeypatch):
    tmp_path.chmod(0o700)
    path = tmp_path / "opaque-input.gpg"
    path.write_bytes(b"opaque-public-input"); path.chmod(0o600)
    plan["source"]["public_key"] = {"path": str(path), "size": path.stat().st_size, "sha256": "f" * 64}
    monkeypatch.setattr(owner, "run_probe", lambda *a, **kw: SimpleNamespace(stdout=b"sec:::::::::\nfpr:::::::::ABCD:\n"))
    monkeypatch.setattr(owner.contract, "stable_read_path", lambda *a, **kw: pytest.fail("secret body must not be parent-read"))
    with pytest.raises(RuntimeError, match="public-only"):
        owner.public_keyring(plan, tmp_path, {})


@pytest.mark.parametrize("change,reason", [("HEAD", "HEAD differs"), ("tree", "tree differs"), ("index", "index differs"), ("dirty", "not clean"), ("branch", "optimizations")])
def test_source_identity_rejected_before_capture(tmp_path, plan, monkeypatch, change, reason):
    tmp_path.chmod(0o700)
    plan["source"]["repo_root"] = str(tmp_path)
    def git(root, *args):
        commands = {("rev-parse", "--show-toplevel"): os.fsencode(tmp_path),
                    ("branch", "--show-current"): b"optimizations", ("rev-parse", "HEAD"): b"a" * 40,
                    ("rev-parse", "a" * 40 + "^{tree}"): b"b" * 40, ("write-tree",): b"b" * 40,
                    ("status", "--porcelain=v1", "--untracked-files=normal"): b""}
        bad = {"HEAD": ("rev-parse", "HEAD"), "tree": ("rev-parse", "a" * 40 + "^{tree}"),
               "index": ("write-tree",), "dirty": ("status", "--porcelain=v1", "--untracked-files=normal"),
               "branch": ("branch", "--show-current")}
        return b"bad" if args == bad[change] else commands[args]
    monkeypatch.setattr(owner, "run_probe", lambda *a, **kw: pytest.fail("bad source must not reach signature/capture"))
    with pytest.raises(RuntimeError, match=reason):
        owner.verify_source(plan, git, {}, tmp_path, "before")


def test_signature_rejects_a_different_full_signer(tmp_path, plan, monkeypatch):
    tmp_path.chmod(0o700); plan["source"]["repo_root"] = str(tmp_path)
    def git(root, *args):
        if args == ("rev-parse", "--show-toplevel"):
            return os.fsencode(tmp_path)
        if args == ("branch", "--show-current"):
            return b"optimizations"
        if args == ("rev-parse", "HEAD"):
            return b"a" * 40
        if args[0] in ("rev-parse", "write-tree"):
            return b"b" * 40
        if args[0] == "status":
            return b""
        pytest.fail("different signature must be rejected")
    monkeypatch.setattr(owner, "run_probe", lambda *a, **kw: SimpleNamespace(stderr=b"[GNUPG:] VALIDSIG " + b"E" * 40 + b" 123 0 0\n"))
    with pytest.raises(RuntimeError, match="full signer"):
        owner.verify_source(plan, git, {}, tmp_path, "before")


def test_pinned_git_program_and_public_keyring_are_explicit(plan, monkeypatch):
    seen = {}
    def child(argv, **kwargs):
        seen.update(argv=argv, **kwargs)
        return SimpleNamespace(returncode=0, stdout=b"answer\n")
    monkeypatch.setattr(owner.subprocess, "run", child)
    assert owner.git_runner(plan, {"GNUPGHOME": "/owned/public-keyring"})(Path("/owned/source"), "rev-parse", "HEAD") == b"answer"
    assert seen["argv"][0] == plan["tools"]["git"]["path"]
    assert "gpg.program=" + plan["tools"]["gpg"]["path"] in seen["argv"]
    assert "--no-replace-objects" in seen["argv"] and seen["env"] == {"GNUPGHOME": "/owned/public-keyring"}


def test_metadata_original_streams_and_offline_closure_are_required(tmp_path, monkeypatch):
    seen = {}
    def probe(argv, env, output, label, **kwargs):
        seen.update(argv=argv, label=label, **kwargs)
        return SimpleNamespace(stdout=json.dumps({"packages": [{"name": "irohad", "source": None},
            {"name": "iroha_cli", "source": None}, {"name": "external", "source": "registry"}]}).encode())
    monkeypatch.setattr(owner, "run_probe", probe)
    assert owner.metadata_packages(Path("/frozen"), {"CARGO": "/pinned/cargo"}, tmp_path, (11, 12)) == {"irohad", "iroha_cli"}
    assert "--offline" in seen["argv"] and "--locked" in seen["argv"] and seen["lock_fds"] == (11, 12)


def test_actual_native_outputs_are_captured_without_changing_cache(tmp_path):
    tmp_path.chmod(0o700)
    target, output = tmp_path / "warm", tmp_path / "attempt"
    target.mkdir(mode=0o700); output.mkdir(mode=0o700)
    (target / "debug").mkdir(mode=0o700)
    originals = {}
    for name, _ in owner.BINARIES:
        path = target / "debug" / name
        path.write_bytes(elf() + name.encode()); path.chmod(0o755)
        originals[name] = path.read_bytes()
    rows = owner.capture_artifacts(target, output)
    assert {row["name"] for row in rows} == set(originals)
    for row in rows:
        assert Path(row["path"]).read_bytes() == originals[row["name"]]
        assert row["sha256"] == owner.sha(originals[row["name"]])
        assert (target / "debug" / row["name"]).read_bytes() == originals[row["name"]]
        assert Path(row["path"]).stat().st_mode & 0o777 == 0o500
    assert (output / "bin").stat().st_mode & 0o777 == 0o500


def test_failed_actual_build_retains_failure_without_manifest(tmp_path, plan, monkeypatch):
    tmp_path.chmod(0o700)
    paths = {}
    for name in ("repo", "oldrepo", "warm", "cargo-home"):
        paths[name] = tmp_path / name; paths[name].mkdir(mode=0o700)
    captured = paths["warm"] / "captured"; captured.mkdir(mode=0o700)
    (captured / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.93.1"\n')
    plan["source"]["repo_root"] = str(paths["repo"])
    plan.update(target_dir=str(paths["warm"]), lane_owner_repo_root=str(paths["oldrepo"]), output_dir=str(tmp_path / "attempt"))
    plan["environment"].update(HOME=owner.pwd.getpwuid(os.geteuid()).pw_dir, CARGO_HOME=str(paths["cargo-home"]),
                              CARGO_TARGET_DIR=str(paths["warm"]), CARGO_ZIGBUILD_ZIG_PATH=str(captured / "scripts/zig_linux_gnu.py"))
    monkeypatch.setattr(owner.sys, "platform", "linux"); monkeypatch.setattr(owner.platform, "machine", lambda: "aarch64")
    @contextlib.contextmanager
    def lane(*args, **kwargs):
        yield 11
    @contextlib.contextmanager
    def source_lane(*args):
        yield captured, 12
    monkeypatch.setattr(owner.release, "cargo_lane", lane)
    monkeypatch.setattr(owner.release, "source_lane", source_lane)
    monkeypatch.setattr(owner, "verify_tools", lambda p: {"actual": "pinned"})
    monkeypatch.setattr(owner, "public_keyring", lambda p, o, e: e)
    monkeypatch.setattr(owner, "verify_source", lambda *args: ({"source": "authenticated"}, b"entries"))
    monkeypatch.setattr(owner, "run_probe", lambda *a, **kw: SimpleNamespace(stdout=b"host: aarch64-unknown-linux-gnu\nrelease: 1.93.1\n"))
    monkeypatch.setattr(owner.release, "signed_source_size", lambda *a: 0)
    monkeypatch.setattr(owner.release, "capture_source", lambda *a: captured)
    monkeypatch.setattr(owner.release, "frozen_snapshot", lambda *a: [{"signed": "source"}])
    monkeypatch.setattr(owner, "metadata_packages", lambda *a: {"irohad", "iroha_cli"})
    monkeypatch.setattr(owner, "carrier_owner_graph", lambda *a: expected_graph(captured))
    monkeypatch.setattr(owner.cache, "admit_source_fingerprints", lambda *a: [])
    def build(cmd, env, out, locks):
        assert locks[0:2] == (11, 12)
        (out / "build.stdout").write_bytes(b"actual failed stdout")
        (out / "build.stderr").write_bytes(b"actual failed compiler stderr")
        return 42
    monkeypatch.setattr(owner, "run_build", build)
    monkeypatch.setattr(owner, "capture_artifacts", lambda *a: pytest.fail("failed Cargo must not capture artifacts"))
    with pytest.raises(RuntimeError, match="build failed"):
        owner.execute(plan, owner.canonical(plan))
    output = Path(plan["output_dir"])
    failure = json.loads((output / "failure.json").read_bytes())
    assert failure["exit_code"] == 42 and failure["stage"] == "cargo"
    assert failure["profile"] == "dev" and failure["checks_run"] is False
    assert failure["native_release_qualified"] is False and failure["application_ready"] is False
    assert "artifacts" not in failure and not (output / "manifest.json").exists()
    assert (output / "build.stderr").read_bytes() == b"actual failed compiler stderr"
    with pytest.raises(RuntimeError, match="output already exists"):
        owner.execute(plan, owner.canonical(plan))
