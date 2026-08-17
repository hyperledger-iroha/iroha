"""Focused tests for guarded, independent Taira validator supervision."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import plistlib
import stat
import subprocess
import sys
import time
import tomllib
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MIGRATION_PATH = REPO_ROOT / "scripts" / "migrate_taira_peer_supervision.py"
SUPERVISOR_PATH = REPO_ROOT / "scripts" / "taira_peer_supervisor.py"
TAIRA_CONFIG_PATH = REPO_ROOT / "configs" / "soranexus" / "taira" / "config.toml"


def load_script(path: Path, name: str) -> ModuleType:
    """Import a standalone script as a test module."""

    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


migration = load_script(MIGRATION_PATH, "migrate_taira_peer_supervision")
supervisor = load_script(SUPERVISOR_PATH, "taira_peer_supervisor")
AUTHENTICATED_NODE_IDS = tuple(
    "taira-node:receipt-signer:secp256k1:sha256:" + f"{number:064x}"
    for number in range(1, 5)
)
AUTHENTICATED_NODE_BINDINGS = tuple(
    f"taira-validator-{number}={AUTHENTICATED_NODE_IDS[number - 1]}"
    for number in range(1, 5)
)


def test_taira_source_profile_carries_browser_query_admission_headroom() -> None:
    """Rendered peers inherit enough authenticated query burst for alias fan-out."""

    config = tomllib.loads(TAIRA_CONFIG_PATH.read_text(encoding="utf-8"))
    assert config["torii"]["query_rate_per_authority_per_sec"] == 160
    assert config["torii"]["query_burst_per_authority"] == 320


def write_file(path: Path, body: str, mode: int = 0o600) -> None:
    """Write one fixture file with an explicit mode."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    path.chmod(mode)


def fake_plan_layout(tmp_path: Path) -> tuple[Path, list[Path], list[Path], list[Path]]:
    """Create a four-peer canonical layout without starting processes."""

    base = tmp_path / "taira"
    binary = base / "bin" / "iroha3d"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    runner = base / "run-canonical.sh"
    write_file(runner, "#!/bin/zsh\nexit 0\n", 0o700)
    write_file(base / "canonical" / "genesis.signed.nrt", "signed-genesis\n")
    configs: list[Path] = []
    storage: list[Path] = []
    pid_files: list[Path] = []
    for index in range(4):
        config = base / "canonical" / f"taira-validator-{index + 1}" / "config.toml"
        write_file(
            config,
            "[torii]\n" f'address = "addr:127.0.0.1:{29080 + index}#0000"\n',
        )
        workdir = base / "storage" / f"peer{index}"
        workdir.mkdir(parents=True)
        (workdir / "kura").mkdir()
        (workdir / "snapshot").mkdir()
        pid_file = base / f"canonical-peer{index}.pid"
        write_file(pid_file, f"{1001 + index}\n")
        configs.append(config)
        storage.append(workdir)
        pid_files.append(pid_file)
    return base, configs, storage, pid_files


def render_fake_plan(
    tmp_path: Path,
) -> tuple[dict[str, object], dict[str, bytes], list[Path], list[Path]]:
    """Render a complete sealed plan from a synthetic legacy process tree."""

    base, configs, storage, _pid_files = fake_plan_layout(tmp_path)
    binary = (base / "bin" / "iroha3d").resolve()
    controller = migration.ProcessIdentity(
        pid=900,
        ppid=1,
        started="Fri Jul 25 10:00:00 2026",
        command=(
            "env TAIRA_PRIVATE_KEY=do-not-persist "
            f"/bin/zsh {base / 'run-canonical.sh'}"
        ),
        cwd=str(base),
    )
    processes = {
        900: controller,
        **{
            1001
            + index: migration.ProcessIdentity(
                pid=1001 + index,
                ppid=900,
                started=f"Fri Jul 25 10:00:0{index + 1} 2026",
                command=f"{binary} --sora --config {configs[index]}",
                cwd=str(configs[index].parent),
            )
            for index in range(4)
        },
    }
    parser = migration.build_parser()
    argv = [
        "plan",
        "--base",
        str(base),
        "--output-dir",
        str(tmp_path / "stage"),
        "--maximum-backoff-seconds",
        "17",
    ]
    for binding in AUTHENTICATED_NODE_BINDINGS:
        argv.extend(("--authenticated-node-binding", binding))
    args = parser.parse_args(argv)
    manifest, assets = migration.create_plan(
        args, process_inspector=lambda pid: processes[pid]
    )
    return manifest, assets, configs, storage


def stat_sealed_supervisor_fixture(
    tmp_path: Path, *, trusted_binary: bool = True
) -> tuple[object, Path, Path]:
    """Create runtime paths and arguments carrying a complete binary stat seal."""

    binary = Path("/usr/bin/true") if trusted_binary else tmp_path / "iroha3d"
    config = tmp_path / "peer.toml"
    workdir = tmp_path / "canonical" / "taira-validator-1"
    storage = tmp_path / "storage" / "peer0"
    if not trusted_binary:
        write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    write_file(config, '[torii]\naddress = "addr:127.0.0.1:29080#0000"\n')
    workdir.mkdir(parents=True)
    storage.mkdir(parents=True)
    binary_info = binary.stat()
    workdir_info = workdir.stat()
    storage_info = storage.stat()
    args = migration.argparse.Namespace(
        binary=str(binary),
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        binary_device=binary_info.st_dev,
        binary_inode=binary_info.st_ino,
        binary_size=binary_info.st_size,
        binary_mtime_ns=binary_info.st_mtime_ns,
        binary_ctime_ns=binary_info.st_ctime_ns,
        config=str(config),
        config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
        workdir=str(workdir),
        workdir_device=workdir_info.st_dev,
        workdir_inode=workdir_info.st_ino,
        storage_dir=str(storage),
        storage_device=storage_info.st_dev,
        storage_inode=storage_info.st_ino,
    )
    return args, binary, config


def supervisor_lifecycle_args(tmp_path: Path) -> list[str]:
    """Build the one mandatory lifecycle identity for a supervisor process test."""

    parent = tmp_path / "lifecycle"
    parent.mkdir(mode=0o700, exist_ok=True)
    parent.chmod(0o700)
    return [
        "--lifecycle-journal-root",
        str(parent / "taira-validator-1"),
        "--validator-id",
        "taira-validator-1",
        "--node-id",
        AUTHENTICATED_NODE_IDS[0],
    ]


def test_supervisor_acl_gate_is_a_stable_noop_off_macos(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Non-macOS restart validation adds no subprocess or payload work."""

    path = tmp_path / "trusted"
    write_file(path, "trusted")
    expected = path.lstat()
    monkeypatch.setattr(supervisor.sys, "platform", "linux")
    monkeypatch.setattr(
        supervisor.subprocess,
        "run",
        lambda *_args, **_kwargs: pytest.fail("non-macOS ACL command ran"),
    )

    actual = supervisor.require_acl_free_path(path, "test trusted path")

    assert supervisor.metadata_identity(actual) == supervisor.metadata_identity(
        expected
    )


def test_supervisor_acl_gate_fails_closed_when_inspector_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A missing or failed absolute ACL inspector cannot enable the stat path."""

    path = tmp_path / "trusted"
    write_file(path, "trusted")
    monkeypatch.setattr(supervisor.sys, "platform", "darwin")
    monkeypatch.setattr(
        supervisor.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            args=[], returncode=1, stdout=b"", stderr=b"inspection failed"
        ),
    )

    with pytest.raises(supervisor.IdentityError, match="extended ACL"):
        supervisor.require_acl_free_path(path, "test trusted path")


@pytest.mark.skipif(sys.platform != "darwin", reason="macOS ACL semantics")
def test_supervisor_acl_gate_rejects_everyone_write(tmp_path: Path) -> None:
    """POSIX mode bits cannot hide an extended write grant."""

    path = tmp_path / "trusted"
    write_file(path, "trusted", 0o400)
    grant = subprocess.run(
        ["/bin/chmod", "+a", "everyone allow write", str(path)],
        check=False,
        capture_output=True,
    )
    assert grant.returncode == 0, grant.stderr.decode(errors="replace")
    try:
        assert path.stat().st_mode & 0o022 == 0
        with pytest.raises(supervisor.IdentityError, match="extended ACL"):
            supervisor.require_acl_free_path(path, "test trusted path")
    finally:
        subprocess.run(
            ["/bin/chmod", "-N", str(path)],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )


def test_plan_renders_four_independent_keepalive_jobs(tmp_path: Path) -> None:
    """Generic adoption emits four full-hash jobs without unsafe stat seals."""

    manifest, assets, configs, storage = render_fake_plan(tmp_path)

    assert manifest["schema_version"] == 4
    assert manifest["runtime"]["binary_stat_sealed"] is False
    assert len(manifest["peers"]) == 4
    assert {peer["label"] for peer in manifest["peers"]} == {
        f"io.soramitsu.taira.validator-{number}" for number in range(1, 5)
    }
    all_arguments: list[list[str]] = []
    for index, peer in enumerate(manifest["peers"]):
        plist = plistlib.loads(assets[f"launchd/{peer['label']}.plist"])
        assert plist["KeepAlive"] is True
        assert plist["RunAtLoad"] is True
        assert plist["AbandonProcessGroup"] is False
        assert plist["WorkingDirectory"] == str(configs[index].parent)
        environment = plist["EnvironmentVariables"]
        assert environment["GENESIS"] == str(
            configs[index].parents[1] / "genesis.signed.nrt"
        )
        assert environment["KURA_STORE_DIR"] == str(storage[index] / "kura")
        assert environment["SNAPSHOT_STORE_DIR"] == str(storage[index] / "snapshot")
        arguments = plist["ProgramArguments"]
        all_arguments.append(arguments)
        for option in (
            "--binary-device",
            "--binary-inode",
            "--binary-size",
            "--binary-mtime-ns",
            "--binary-ctime-ns",
        ):
            assert option not in arguments
        assert (
            arguments[arguments.index("--binary-sha256") + 1]
            == manifest["binary"]["sha256"]
        )
        assert arguments[arguments.index("--config") + 1] == str(configs[index])
        assert arguments[arguments.index("--workdir") + 1] == str(configs[index].parent)
        assert arguments[arguments.index("--storage-dir") + 1] == str(storage[index])
        assert arguments[arguments.index("--maximum-backoff-seconds") + 1] == "17.0"
        assert arguments[arguments.index("--terminal-unhealthy-file") + 1].endswith(
            f"/terminal/validator-{index + 1}-terminal-unhealthy.json"
        )
        assert (
            arguments[arguments.index("--restart-generation") + 1]
            == manifest["runtime"]["restart_generation"]
        )
        assert migration.is_sha256(manifest["runtime"]["restart_generation"])
        assert arguments[arguments.index("--lifecycle-journal-root") + 1] == str(
            Path(manifest["install"]["directory"])
            / "lifecycle"
            / f"taira-validator-{index + 1}"
        )
        assert (
            arguments[arguments.index("--validator-id") + 1]
            == f"taira-validator-{index + 1}"
        )
        assert (
            arguments[arguments.index("--node-id") + 1]
            == AUTHENTICATED_NODE_IDS[index]
        )
        assert str(configs[(index + 1) % 4]) not in arguments
    assert len({tuple(arguments) for arguments in all_arguments}) == 4
    assert "do-not-persist" not in json.dumps(manifest)
    assert manifest["genesis"]["path"] == str(
        configs[0].parents[1] / "genesis.signed.nrt"
    )
    assert migration.is_sha256(manifest["legacy"]["controller"]["command_sha256"])


def test_plist_requires_the_manifest_bound_lifecycle_identity(
    tmp_path: Path,
) -> None:
    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    peer = manifest["peers"][0]
    install_dir = Path(manifest["install"]["directory"])
    root = migration.lifecycle_journal_root(install_dir, 1)

    payload = migration.launchd_plist(
        peer=peer,
        manifest=manifest,
        installed_supervisor=Path(manifest["install"]["supervisor"]),
        python_path=Path(manifest["python"]["path"]),
    )
    arguments = plistlib.loads(payload)["ProgramArguments"]
    assert arguments[arguments.index("--lifecycle-journal-root") + 1] == str(root)
    assert arguments[arguments.index("--validator-id") + 1] == "taira-validator-1"
    assert arguments[arguments.index("--node-id") + 1] == AUTHENTICATED_NODE_IDS[0]

    peer["lifecycle"]["node_id"] = "not canonical spaces"
    with pytest.raises(migration.MigrationError, match="authenticated node ID"):
        migration.launchd_plist(
            peer=peer,
            manifest=manifest,
            installed_supervisor=Path(manifest["install"]["supervisor"]),
            python_path=Path(manifest["python"]["path"]),
        )

    peer["lifecycle"]["node_id"] = AUTHENTICATED_NODE_IDS[0]
    peer["lifecycle"]["journal_root"] = str(tmp_path / "alternate")
    with pytest.raises(migration.MigrationError, match="journal path"):
        migration.launchd_plist(
            peer=peer,
            manifest=manifest,
            installed_supervisor=Path(manifest["install"]["supervisor"]),
            python_path=Path(manifest["python"]["path"]),
        )


@pytest.mark.parametrize(
    ("values", "message"),
    (
        (None, "exactly once"),
        (AUTHENTICATED_NODE_BINDINGS[:3], "exactly once"),
        (
            (*AUTHENTICATED_NODE_BINDINGS[:3], AUTHENTICATED_NODE_BINDINGS[0]),
            "repeats validator slug",
        ),
        (
            (
                *AUTHENTICATED_NODE_BINDINGS[:3],
                f"taira-validator-5={AUTHENTICATED_NODE_IDS[3]}",
            ),
            "taira-validator-N",
        ),
        (
            (*AUTHENTICATED_NODE_BINDINGS[:3], "taira-validator-4=not-canonical"),
            "not canonical",
        ),
        (
            (
                *AUTHENTICATED_NODE_BINDINGS[:3],
                f"taira-validator-4={AUTHENTICATED_NODE_IDS[0]}",
            ),
            "must be distinct",
        ),
    ),
)
def test_plan_requires_exact_authenticated_node_bindings(
    values: tuple[str, ...] | None,
    message: str,
) -> None:
    with pytest.raises(migration.MigrationError, match=message):
        migration.require_authenticated_node_bindings(values)


def test_authenticated_node_bindings_return_canonical_peer_order() -> None:
    assert (
        migration.require_authenticated_node_bindings(
            tuple(reversed(AUTHENTICATED_NODE_BINDINGS))
        )
        == AUTHENTICATED_NODE_IDS
    )


def test_manifest_rejects_missing_or_rebound_lifecycle_identity(tmp_path: Path) -> None:
    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    manifest["peers"][0]["lifecycle"]["node_id"] = AUTHENTICATED_NODE_IDS[1]
    with pytest.raises(migration.MigrationError, match="duplicate lifecycle node ID"):
        migration.validate_manifest_shape(manifest)

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path / "missing")
    del manifest["peers"][0]["lifecycle"]
    with pytest.raises(migration.MigrationError, match="manifest structure is invalid"):
        migration.validate_manifest_shape(manifest)


def test_migration_lifecycle_layout_is_fixed_distinct_and_owner_private(
    tmp_path: Path,
) -> None:
    install_dir = tmp_path / "supervision"
    install_dir.mkdir(mode=0o700)
    uid, gid = os.getuid(), os.getgid()

    roots = migration.ensure_lifecycle_journal_layout(
        install_dir, uid=uid, gid=gid
    )

    assert roots == tuple(
        install_dir / "lifecycle" / f"taira-validator-{number}"
        for number in range(1, 5)
    )
    assert len(set(roots)) == 4
    for root in roots:
        info = root.lstat()
        assert stat.S_ISDIR(info.st_mode)
        assert stat.S_IMODE(info.st_mode) == 0o700
        assert (info.st_uid, info.st_gid) == (uid, gid)


def test_pre_latch_v3_manifest_has_an_explicit_version_boundary(
    tmp_path: Path,
) -> None:
    """A sealed v3 plan cannot be interpreted under the required latch contract."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    manifest["schema_version"] = 3
    with pytest.raises(
        migration.MigrationError,
        match="unsupported supervision manifest schema",
    ):
        migration.validate_manifest_shape(manifest)


def test_root_controlled_binary_plist_carries_complete_fast_stat_seal(
    tmp_path: Path,
) -> None:
    """A trusted binary never falls back to a full read on child restart."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    binary = Path("/usr/bin/true")
    migration.require_root_controlled_executable_chain(binary)
    manifest["binary"] = migration.file_identity(binary, executable=True).as_dict()
    manifest["runtime"]["binary_stat_sealed"] = True
    peer = manifest["peers"][0]
    payload = migration.launchd_plist(
        peer=peer,
        manifest=manifest,
        installed_supervisor=Path(manifest["install"]["supervisor"]),
        python_path=Path(manifest["python"]["path"]),
    )
    arguments = plistlib.loads(payload)["ProgramArguments"]
    expected = {
        "--binary-device": "device",
        "--binary-inode": "inode",
        "--binary-size": "size",
        "--binary-mtime-ns": "mtime_ns",
        "--binary-ctime-ns": "ctime_ns",
    }
    for option, field in expected.items():
        assert arguments.count(option) == 1
        assert arguments[arguments.index(option) + 1] == str(manifest["binary"][field])


def test_runtime_writable_binary_is_not_eligible_for_fast_stat_seal(
    tmp_path: Path,
) -> None:
    """The migration retains full hashing when a runtime user can swap the path."""

    binary = tmp_path / "iroha3d"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    assert migration.binary_supports_fast_stat_seal(binary) is False


def test_manifest_rejects_store_paths_not_derived_from_sealed_root(
    tmp_path: Path,
) -> None:
    """A sealed plan cannot redirect Kura outside its adopted peer storage root."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    manifest["peers"][0]["stores"]["kura"]["path"] = str(tmp_path / "redirected-kura")
    with pytest.raises(
        migration.MigrationError,
        match="store paths are inconsistent with storage root",
    ):
        migration.validate_manifest_shape(manifest)


@pytest.mark.parametrize(
    "command",
    [
        "/bin/zsh /srv/taira/run-canonical.sh",
        "/bin/zsh -lc 'cd /srv/taira && ./launchd-run.sh'",
    ],
)
def test_legacy_controller_guard_accepts_only_named_approved_runner(
    command: str,
) -> None:
    """Both known legacy launchers are explicit migration inputs."""

    process = migration.ProcessIdentity(
        pid=90,
        ppid=1,
        started="Fri Jul 25 10:00:00 2026",
        command=command,
    )
    selected = migration.require_legacy_controller_command(
        process,
        Path("/srv/taira"),
        [
            Path("/srv/taira/run-canonical.sh"),
            Path("/srv/taira/launchd-run.sh"),
        ],
    )
    assert selected.name in command


def test_peer_command_guard_rejects_extra_or_changed_arguments() -> None:
    """PID reuse or a changed config cannot pass an approximate command match."""

    binary = Path("/srv/taira/bin/iroha3d")
    config = Path("/srv/taira/peer0.toml")
    exact = migration.ProcessIdentity(
        pid=101,
        ppid=90,
        started="Fri Jul 25 10:00:01 2026",
        command=f"{binary} --sora --config {config}",
    )
    migration.require_peer_command(exact, binary, config)
    changed = migration.dataclasses.replace(
        exact, command=f"{exact.command} --unexpected"
    )
    with pytest.raises(migration.MigrationError, match="command mismatch"):
        migration.require_peer_command(changed, binary, config)


def test_path_identity_guard_rejects_replaced_storage_inode(tmp_path: Path) -> None:
    """Adoption is tied to the exact existing storage directory, not its name."""

    storage = tmp_path / "storage"
    storage.mkdir()
    planned = migration.directory_identity(storage).as_dict()
    storage.rmdir()
    storage.mkdir()
    with pytest.raises(migration.MigrationError, match="identity changed"):
        migration.require_path_unchanged(planned)


def test_path_identity_guard_rejects_symlinked_config(tmp_path: Path) -> None:
    """Config hashing never follows a substituted symlink."""

    target = tmp_path / "target.toml"
    target.write_text("[torii]\n", encoding="utf-8")
    link = tmp_path / "config.toml"
    link.symlink_to(target)
    with pytest.raises(migration.MigrationError, match="non-symlink regular file"):
        migration.file_identity(link)


def test_supervisor_guards_working_directory_and_storage_independently(
    tmp_path: Path,
) -> None:
    """Preserving the legacy cwd does not weaken the separate storage-inode seal."""

    binary = tmp_path / "iroha3d"
    config = tmp_path / "peer.toml"
    workdir = tmp_path / "canonical" / "taira-validator-1"
    storage = tmp_path / "storage" / "peer0"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    write_file(config, '[torii]\naddress = "addr:127.0.0.1:29080#0000"\n')
    workdir.mkdir(parents=True)
    storage.mkdir(parents=True)
    workdir_stat = workdir.stat()
    storage_stat = storage.stat()
    args = migration.argparse.Namespace(
        binary=str(binary),
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        config=str(config),
        config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
        workdir=str(workdir),
        workdir_device=workdir_stat.st_dev,
        workdir_inode=workdir_stat.st_ino,
        storage_dir=str(storage),
        storage_device=storage_stat.st_dev,
        storage_inode=storage_stat.st_ino,
    )
    supervisor.require_runtime_identity(args)
    storage.rmdir()
    storage.mkdir()
    with pytest.raises(supervisor.IdentityError, match="storage directory identity"):
        supervisor.require_runtime_identity(args)


def test_supervisor_stat_seal_skips_binary_hash_but_still_hashes_config(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Restart validation is O(1) for the large binary and authenticates config."""

    args, binary, config = stat_sealed_supervisor_fixture(tmp_path)
    real_sha256_file = supervisor.sha256_file
    hashed_paths: list[Path] = []

    def monitored_sha256_file(path: Path) -> str:
        hashed_paths.append(path)
        if path == binary:
            raise AssertionError("stat-sealed binary must not be read")
        return real_sha256_file(path)

    monkeypatch.setattr(supervisor, "sha256_file", monitored_sha256_file)
    supervisor.require_runtime_identity(args)
    assert hashed_paths == [config]


def test_supervisor_legacy_without_stat_seal_hashes_binary_and_config(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A legacy plist with no stat fields retains full binary authentication."""

    args, binary, config = stat_sealed_supervisor_fixture(
        tmp_path, trusted_binary=False
    )
    for field in supervisor.BINARY_STAT_SEAL_FIELDS:
        delattr(args, field)
    real_sha256_file = supervisor.sha256_file
    hashed_paths: list[Path] = []

    def monitored_sha256_file(path: Path) -> str:
        hashed_paths.append(path)
        return real_sha256_file(path)

    monkeypatch.setattr(supervisor, "sha256_file", monitored_sha256_file)
    supervisor.require_runtime_identity(args)
    assert hashed_paths == [binary, config]


def test_supervisor_stat_seal_rejects_same_size_mutation_with_restored_mtime(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The ctime seal catches content replacement hidden behind size and mtime."""

    args, binary, _config = stat_sealed_supervisor_fixture(
        tmp_path, trusted_binary=False
    )
    monkeypatch.setattr(supervisor, "require_trusted_binary_path", lambda _path: None)
    planned = binary.stat()
    time.sleep(0.01)
    binary.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    os.utime(binary, ns=(planned.st_atime_ns, planned.st_mtime_ns))
    changed = binary.stat()
    assert changed.st_size == planned.st_size
    assert changed.st_mtime_ns == planned.st_mtime_ns
    assert changed.st_ctime_ns != planned.st_ctime_ns

    with pytest.raises(
        supervisor.IdentityError, match="validator binary stat identity changed"
    ):
        supervisor.require_runtime_identity(args)


def test_supervisor_stat_seal_rejects_runtime_writable_binary_path(
    tmp_path: Path,
) -> None:
    """Fast validation is unavailable when the runtime user can swap the path."""

    args, _binary, _config = stat_sealed_supervisor_fixture(
        tmp_path, trusted_binary=False
    )
    with pytest.raises(
        supervisor.IdentityError,
        match="not root-owned|group/world writable",
    ):
        supervisor.require_runtime_identity(args)


def test_supervisor_refuses_partial_binary_stat_seal(tmp_path: Path) -> None:
    """Mixed old/new plist arguments cannot silently select either identity mode."""

    args, _binary, _config = stat_sealed_supervisor_fixture(tmp_path)
    for field in supervisor.BINARY_STAT_SEAL_FIELDS[1:]:
        delattr(args, field)
    with pytest.raises(
        supervisor.IdentityError,
        match="binary stat seal fields must be provided together",
    ):
        supervisor.require_runtime_identity(args)

    cli = [
        "--binary",
        "/tmp/iroha3d",
        "--binary-sha256",
        "0" * 64,
        "--binary-device",
        "1",
        "--config",
        "/tmp/config.toml",
        "--config-sha256",
        "1" * 64,
        "--workdir",
        "/tmp/workdir",
        "--workdir-device",
        "1",
        "--workdir-inode",
        "2",
        "--storage-dir",
        "/tmp/storage",
        "--storage-device",
        "1",
        "--storage-inode",
        "3",
        "--pid-file",
        "/tmp/peer.pid",
    ]
    with pytest.raises(SystemExit):
        supervisor.parse_args(cli)


def test_apply_requires_exact_high_friction_confirmation() -> None:
    """An apply typo refuses before root checks, reads, or external mutation."""

    args = migration.build_parser().parse_args(
        [
            "apply",
            "--manifest",
            "/does/not/exist",
            "--expected-manifest-sha256",
            "0" * 64,
            "--confirm",
            "yes",
        ]
    )
    with pytest.raises(migration.MigrationError, match="no changes were made"):
        migration.apply_plan(args)


def test_manifest_digest_and_asset_seals_detect_staged_tampering(
    tmp_path: Path,
) -> None:
    """The apply phase pins authenticated bytes and rejects later staged tampering."""

    manifest, assets, _configs, _storage = render_fake_plan(tmp_path)
    stage = tmp_path / "stage"
    digest = migration.write_plan(stage, manifest, assets)
    manifest_path = stage / "manifest.json"
    _loaded_manifest, _loaded_stage, authenticated_assets = migration.read_manifest(
        manifest_path, digest
    )
    asset = stage / migration.SUPERVISOR_SOURCE.name
    expected_asset = authenticated_assets[migration.SUPERVISOR_SOURCE.name]
    asset.write_bytes(b"tampered")
    installed = tmp_path / "installed-supervisor.py"
    migration.copy_new_file(
        expected_asset,
        installed,
        uid=os.getuid(),
        gid=os.getgid(),
        mode=0o700,
    )
    assert installed.read_bytes() == expected_asset
    with pytest.raises(migration.MigrationError, match="asset identity changed"):
        migration.read_manifest(manifest_path, digest)


def test_manifest_rejects_asset_path_traversal(tmp_path: Path) -> None:
    """A root apply cannot be redirected outside the sealed staging directory."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    manifest["assets"]["../escape.plist"] = manifest["assets"].pop(
        next(name for name in manifest["assets"] if name.startswith("launchd/"))
    )
    with pytest.raises(
        migration.MigrationError, match="asset set does not match|unsafe staged asset"
    ):
        migration.validate_manifest_shape(manifest)


def _terminal_binding_args(
    *,
    binary_sha256: str = "a" * 64,
    config_sha256: str = "b" * 64,
    restart_generation: str = "c" * 64,
) -> object:
    """Build the redaction-safe fields bound into one terminal latch."""

    return supervisor.argparse.Namespace(
        binary_sha256=binary_sha256,
        binary_device=1,
        binary_inode=2,
        binary_size=3,
        binary_mtime_ns=4,
        binary_ctime_ns=5,
        config_sha256=config_sha256,
        restart_generation=restart_generation,
    )


def test_fatal_normalization_redacts_dynamic_values_and_distinguishes_reason() -> None:
    """Paths, IDs, and key-shaped tokens do not defeat identical-fatal counting."""

    first = supervisor.normalize_fatal_exit(
        70,
        0.5,
        5.0,
        (
            b"FATAL validator config rejected at /private/run/peer-1.toml "
            b"pid=417 key=0123456789abcdef0123456789abcdef\n"
        ),
    )
    second = supervisor.normalize_fatal_exit(
        70,
        0.8,
        5.0,
        (
            b"fatal validator config rejected at /other/run/peer-9.toml "
            b"pid=991 key=fedcba9876543210fedcba9876543210\n"
        ),
    )
    distinct = supervisor.normalize_fatal_exit(
        70,
        0.8,
        5.0,
        b"fatal validator database is corrupt\n",
    )

    assert first is not None
    assert first == second
    assert distinct is not None and distinct != first
    assert supervisor.normalize_fatal_exit(0, 0.1, 5.0, b"fatal ignored\n") is None
    assert supervisor.normalize_fatal_exit(70, 5.1, 5.0, b"fatal slow\n") is None
    assert supervisor.normalize_fatal_exit(70, 0.1, 5.0, b"warning only\n") is None


def test_irohad_error_shape_normalizes_lane_geometry_startup_refusal() -> None:
    """Rust tracing ERROR lines reproduce one lane-geometry fatal fingerprint."""

    first = supervisor.normalize_fatal_exit(
        1,
        0.4,
        5.0,
        (
            b"2026-07-30T09:41:15.004921Z ERROR iroha_core::sumeragi: "
            b"authoritative geometry identity does not match its exact transition "
            b"cursor error=NonCanonicalSnapshotPayload height=104 "
            b"config=/private/taira/validator-1/config.toml\n"
        ),
    )
    second = supervisor.normalize_fatal_exit(
        1,
        0.7,
        5.0,
        (
            b"2026-07-30T10:55:59.771003Z ERROR iroha_core::sumeragi: "
            b"authoritative geometry identity does not match its exact transition "
            b"cursor error=NonCanonicalSnapshotPayload height=991 "
            b"config=/different/root/validator-4/config.toml\n"
        ),
    )
    different = supervisor.normalize_fatal_exit(
        1,
        0.7,
        5.0,
        (
            b"2026-07-30T10:55:59Z ERROR iroha_core::sumeragi: "
            b"network socket bind refused\n"
        ),
    )

    assert first is not None
    assert first == second
    assert different is not None and different != first


def test_three_identical_rapid_fatals_latch_but_transients_reset() -> None:
    """Only three consecutive identical normalized fatal exits close the loop."""

    tracker = supervisor.RapidFatalExitTracker()
    first = "1" * 64
    second = "2" * 64

    assert tracker.observe(first) is False
    assert tracker.observe(second) is False
    assert tracker.observe(second) is False
    assert tracker.observe(None) is False
    assert tracker.observe(first) is False
    assert tracker.observe(first) is False
    assert tracker.observe(first) is True


def test_terminal_latch_is_atomic_private_bounded_and_restart_persistent(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A three-hit latch is durable, payload-free, and survives supervisor restart."""

    terminal_dir = tmp_path / "terminal"
    terminal_dir.mkdir(mode=0o700)
    terminal_dir.chmod(0o700)
    marker = terminal_dir / "validator-1.json"
    binding = supervisor.terminal_binding_sha256(_terminal_binding_args())
    fatal = "d" * 64
    directory_fsyncs: list[Path] = []
    acl_clears: list[Path] = []
    real_fsync_directory = supervisor.fsync_directory
    real_clear_inherited_acl = supervisor.clear_inherited_acl

    def monitored_fsync_directory(path: Path) -> None:
        directory_fsyncs.append(path)
        real_fsync_directory(path)

    monkeypatch.setattr(supervisor, "fsync_directory", monitored_fsync_directory)

    def monitored_acl_clear(path: Path, expected: os.stat_result, label: str) -> None:
        acl_clears.append(path)
        real_clear_inherited_acl(path, expected, label)

    monkeypatch.setattr(
        supervisor,
        "clear_inherited_acl",
        monitored_acl_clear,
    )
    published = supervisor.publish_terminal_payload(marker, binding, fatal)

    assert stat.S_IMODE(published.st_mode) == 0o600
    assert published.st_uid == os.geteuid()
    assert published.st_nlink == 1
    assert published.st_size <= supervisor.TERMINAL_UNHEALTHY_MAX_BYTES
    assert directory_fsyncs.count(terminal_dir) >= 2
    assert len(acl_clears) == 1
    assert acl_clears[0].parent == terminal_dir
    assert list(terminal_dir.iterdir()) == [marker]
    body = marker.read_text(encoding="ascii")
    assert "config rejected" not in body
    assert "/private/" not in body
    assert supervisor.existing_terminal_latch(marker, binding) is True
    assert supervisor.existing_terminal_latch(marker, binding) is True


def test_terminal_latch_resets_on_generation_config_or_binary_identity(
    tmp_path: Path,
) -> None:
    """Every explicit reset dimension durably removes an old persisted latch."""

    terminal_dir = tmp_path / "terminal"
    terminal_dir.mkdir(mode=0o700)
    terminal_dir.chmod(0o700)
    marker = terminal_dir / "validator-1.json"
    base = _terminal_binding_args()
    binding = supervisor.terminal_binding_sha256(base)
    supervisor.publish_terminal_payload(marker, binding, "d" * 64)

    changed_generation = supervisor.terminal_binding_sha256(
        _terminal_binding_args(restart_generation="e" * 64)
    )
    assert supervisor.existing_terminal_latch(marker, changed_generation) is False
    assert not marker.exists()

    supervisor.publish_terminal_payload(marker, changed_generation, "d" * 64)
    changed_config = supervisor.terminal_binding_sha256(
        _terminal_binding_args(
            config_sha256="f" * 64,
            restart_generation="e" * 64,
        )
    )
    assert supervisor.existing_terminal_latch(marker, changed_config) is False
    assert not marker.exists()

    supervisor.publish_terminal_payload(marker, changed_config, "d" * 64)
    changed_binary = supervisor.terminal_binding_sha256(
        _terminal_binding_args(
            binary_sha256="0" * 64,
            config_sha256="f" * 64,
            restart_generation="e" * 64,
        )
    )
    assert supervisor.existing_terminal_latch(marker, changed_binary) is False
    assert not marker.exists()


def test_terminal_publication_exactly_rolls_back_failed_post_link_verification(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A failed final verification removes only the inode this call published."""

    terminal_dir = tmp_path / "terminal"
    terminal_dir.mkdir(mode=0o700)
    terminal_dir.chmod(0o700)
    marker = terminal_dir / "validator-1.json"
    binding = supervisor.terminal_binding_sha256(_terminal_binding_args())
    real_read = supervisor.read_terminal_payload

    def fail_after_link(path: Path) -> object:
        if path.exists():
            raise supervisor.IdentityError("injected post-link verification failure")
        return real_read(path)

    monkeypatch.setattr(supervisor, "read_terminal_payload", fail_after_link)
    with pytest.raises(
        supervisor.IdentityError,
        match="injected post-link verification failure",
    ):
        supervisor.publish_terminal_payload(marker, binding, "d" * 64)

    assert not marker.exists()
    assert list(terminal_dir.iterdir()) == []


def test_supervisor_three_hit_latch_stays_alive_and_survives_restart(
    tmp_path: Path,
) -> None:
    """Three real identical fatal children stop respawn without exiting launchd's job."""

    binary = tmp_path / "fatal-validator"
    write_file(
        binary,
        "#!/bin/sh\n"
        "echo '2026-07-30T09:41:15Z ERROR iroha_core::sumeragi: "
        "authoritative geometry identity does not match its exact transition "
        "cursor error=NonCanonicalSnapshotPayload height=104' >&2\n"
        "exit 1\n",
        0o700,
    )
    config = tmp_path / "peer.toml"
    write_file(config, '[torii]\naddress = "addr:127.0.0.1:29080#0000"\n')
    workdir = tmp_path / "runtime" / "peer0"
    workdir.mkdir(parents=True)
    storage = tmp_path / "storage" / "peer0"
    storage.mkdir(parents=True)
    terminal = tmp_path / "terminal" / "peer.json"
    pid_file = tmp_path / "peer.pid"
    workdir_info = workdir.stat()
    storage_info = storage.stat()
    command = [
        sys.executable,
        str(SUPERVISOR_PATH),
        "--binary",
        str(binary),
        "--binary-sha256",
        hashlib.sha256(binary.read_bytes()).hexdigest(),
        "--config",
        str(config),
        "--config-sha256",
        hashlib.sha256(config.read_bytes()).hexdigest(),
        "--workdir",
        str(workdir),
        "--workdir-device",
        str(workdir_info.st_dev),
        "--workdir-inode",
        str(workdir_info.st_ino),
        "--storage-dir",
        str(storage),
        "--storage-device",
        str(storage_info.st_dev),
        "--storage-inode",
        str(storage_info.st_ino),
        "--pid-file",
        str(pid_file),
        "--terminal-unhealthy-file",
        str(terminal),
        "--restart-generation",
        "9" * 64,
        *supervisor_lifecycle_args(tmp_path),
        "--initial-backoff-seconds",
        "0.01",
        "--maximum-backoff-seconds",
        "0.02",
        "--stable-uptime-seconds",
        "10",
        "--rapid-fatal-uptime-seconds",
        "1",
    ]
    stdout_path = tmp_path / "supervisor.stdout"
    stderr_path = tmp_path / "supervisor.stderr"

    with stdout_path.open("w", encoding="utf-8") as stdout_stream, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr_stream:
        process = subprocess.Popen(
            command,
            stdout=stdout_stream,
            stderr=stderr_stream,
            text=True,
        )
        try:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and not terminal.exists():
                if process.poll() is not None:
                    pytest.fail("supervisor exited before publishing its fatal latch")
                time.sleep(0.01)
            assert terminal.exists()
            time.sleep(0.1)
            stdout_stream.flush()
            assert process.poll() is None
            assert (
                stdout_path.read_text(encoding="utf-8").count("taira validator started")
                == 3
            )
            assert not pid_file.exists()
        finally:
            process.terminate()
            process.wait(timeout=3)
    assert process.returncode == 0

    second_stdout = tmp_path / "supervisor-restart.stdout"
    second_stderr = tmp_path / "supervisor-restart.stderr"
    with second_stdout.open("w", encoding="utf-8") as stdout_stream, second_stderr.open(
        "w", encoding="utf-8"
    ) as stderr_stream:
        restarted = subprocess.Popen(
            command,
            stdout=stdout_stream,
            stderr=stderr_stream,
            text=True,
        )
        try:
            time.sleep(0.2)
            assert restarted.poll() is None
        finally:
            restarted.terminate()
            restarted.wait(timeout=3)
    assert restarted.returncode == 0
    assert "taira validator started" not in second_stdout.read_text(encoding="utf-8")


def test_supervisor_signal_shutdown_forwards_to_active_child(
    tmp_path: Path,
) -> None:
    """A launchd stop still terminates the exact active child and exits cleanly."""

    binary = tmp_path / "waiting-validator"
    write_file(
        binary,
        "#!/bin/sh\n"
        "trap 'exit 0' TERM INT HUP\n"
        "while :; do /bin/sleep 0.05; done\n",
        0o700,
    )
    config = tmp_path / "peer.toml"
    write_file(config, '[torii]\naddress = "addr:127.0.0.1:29080#0000"\n')
    workdir = tmp_path / "runtime" / "peer0"
    workdir.mkdir(parents=True)
    storage = tmp_path / "storage" / "peer0"
    storage.mkdir(parents=True)
    pid_file = tmp_path / "peer.pid"
    terminal = tmp_path / "terminal" / "peer.json"
    workdir_info = workdir.stat()
    storage_info = storage.stat()
    process = subprocess.Popen(
        [
            sys.executable,
            str(SUPERVISOR_PATH),
            "--binary",
            str(binary),
            "--binary-sha256",
            hashlib.sha256(binary.read_bytes()).hexdigest(),
            "--config",
            str(config),
            "--config-sha256",
            hashlib.sha256(config.read_bytes()).hexdigest(),
            "--workdir",
            str(workdir),
            "--workdir-device",
            str(workdir_info.st_dev),
            "--workdir-inode",
            str(workdir_info.st_ino),
            "--storage-dir",
            str(storage),
            "--storage-device",
            str(storage_info.st_dev),
            "--storage-inode",
            str(storage_info.st_ino),
            "--pid-file",
            str(pid_file),
            "--terminal-unhealthy-file",
            str(terminal),
            "--restart-generation",
            "9" * 64,
            *supervisor_lifecycle_args(tmp_path),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline and not pid_file.exists():
            if process.poll() is not None:
                pytest.fail("supervisor exited before publishing its child PID")
            time.sleep(0.01)
        assert pid_file.exists()
        child_pid = int(pid_file.read_text(encoding="ascii"))
        process.terminate()
        process.wait(timeout=3)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=3)

    assert process.returncode == 0
    assert not pid_file.exists()
    assert not terminal.exists()
    with pytest.raises(ProcessLookupError):
        os.kill(child_pid, 0)


def test_single_peer_supervisor_uses_bounded_restart_backoff(
    tmp_path: Path,
) -> None:
    """A crashing validator restarts locally and never exceeds the configured cap."""

    # Use the platform's sealed, root-owned `false` executable. Executing a
    # freshly written script is denied by some macOS CI sandboxes before its
    # first instruction, which would test the sandbox rather than backoff.
    binary = Path("/usr/bin/false")
    binary_info = binary.stat()
    config = tmp_path / "peer.toml"
    write_file(config, '[torii]\naddress = "addr:127.0.0.1:29080#0000"\n')
    workdir = tmp_path / "runtime" / "peer0"
    workdir.mkdir(parents=True)
    storage = tmp_path / "storage" / "peer0"
    storage.mkdir(parents=True)
    workdir_info = workdir.stat()
    storage_info = storage.stat()
    stdout_path = tmp_path / "supervisor.stdout"
    stderr_path = tmp_path / "supervisor.stderr"
    with stdout_path.open("w", encoding="utf-8") as stdout_stream, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr_stream:
        process = subprocess.Popen(
            [
                sys.executable,
                str(SUPERVISOR_PATH),
                "--binary",
                str(binary),
                "--binary-sha256",
                hashlib.sha256(binary.read_bytes()).hexdigest(),
                "--binary-device",
                str(binary_info.st_dev),
                "--binary-inode",
                str(binary_info.st_ino),
                "--binary-size",
                str(binary_info.st_size),
                "--binary-mtime-ns",
                str(binary_info.st_mtime_ns),
                "--binary-ctime-ns",
                str(binary_info.st_ctime_ns),
                "--config",
                str(config),
                "--config-sha256",
                hashlib.sha256(config.read_bytes()).hexdigest(),
                "--workdir",
                str(workdir),
                "--workdir-device",
                str(workdir_info.st_dev),
                "--workdir-inode",
                str(workdir_info.st_ino),
                "--storage-dir",
                str(storage),
                "--storage-device",
                str(storage_info.st_dev),
                "--storage-inode",
                str(storage_info.st_ino),
                "--pid-file",
                str(tmp_path / "peer.pid"),
                "--terminal-unhealthy-file",
                str(tmp_path / "terminal" / "peer.json"),
                "--restart-generation",
                "9" * 64,
                *supervisor_lifecycle_args(tmp_path),
                "--initial-backoff-seconds",
                "0.02",
                "--maximum-backoff-seconds",
                "0.04",
                "--stable-uptime-seconds",
                "10",
            ],
            stdout=stdout_stream,
            stderr=stderr_stream,
            text=True,
        )
        try:
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                stderr_stream.flush()
                if (
                    stderr_path.read_text(encoding="utf-8").count("restart_in_seconds=")
                    >= 4
                ):
                    break
                if process.poll() is not None:
                    pytest.fail(
                        "single-peer supervisor exited before the restart sample completed"
                    )
                time.sleep(0.01)
        finally:
            process.terminate()
            process.wait(timeout=3)
    stdout = stdout_path.read_text(encoding="utf-8")
    stderr = stderr_path.read_text(encoding="utf-8")
    restart_delays = [
        float(line.rsplit("restart_in_seconds=", 1)[1])
        for line in stderr.splitlines()
        if "restart_in_seconds=" in line
    ]
    assert len(restart_delays) >= 4
    assert restart_delays[:3] == [0.02, 0.04, 0.04]
    assert max(restart_delays) <= 0.04
    assert "taira validator started" in stdout
