"""Focused tests for guarded, independent Taira validator supervision."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import plistlib
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
    binary = base / "bin" / "irohad"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    runner = base / "run-canonical.sh"
    write_file(runner, "#!/bin/zsh\nexit 0\n", 0o700)
    write_file(base / "canonical" / "genesis.signed.nrt", "signed-genesis\n")
    configs: list[Path] = []
    storage: list[Path] = []
    pid_files: list[Path] = []
    for index in range(4):
        config = (
            base
            / "canonical"
            / f"taira-validator-{index + 1}"
            / "config.toml"
        )
        write_file(
            config,
            "[torii]\n"
            f'address = "addr:127.0.0.1:{29080 + index}#0000"\n',
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
    binary = (base / "bin" / "irohad").resolve()
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
            1001 + index: migration.ProcessIdentity(
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
    args = parser.parse_args(
        [
            "plan",
            "--base",
            str(base),
            "--output-dir",
            str(tmp_path / "stage"),
            "--maximum-backoff-seconds",
            "17",
        ]
    )
    manifest, assets = migration.create_plan(
        args, process_inspector=lambda pid: processes[pid]
    )
    return manifest, assets, configs, storage


def stat_sealed_supervisor_fixture(
    tmp_path: Path, *, trusted_binary: bool = True
) -> tuple[object, Path, Path]:
    """Create runtime paths and arguments carrying a complete binary stat seal."""

    binary = Path("/usr/bin/true") if trusted_binary else tmp_path / "irohad"
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


def test_plan_renders_four_independent_keepalive_jobs(tmp_path: Path) -> None:
    """Generic adoption emits four full-hash jobs without unsafe stat seals."""

    manifest, assets, configs, storage = render_fake_plan(tmp_path)

    assert manifest["schema_version"] == 3
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
        assert environment["SNAPSHOT_STORE_DIR"] == str(
            storage[index] / "snapshot"
        )
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
        assert arguments[arguments.index("--binary-sha256") + 1] == manifest[
            "binary"
        ]["sha256"]
        assert arguments[arguments.index("--config") + 1] == str(configs[index])
        assert (
            arguments[arguments.index("--workdir") + 1]
            == str(configs[index].parent)
        )
        assert (
            arguments[arguments.index("--storage-dir") + 1]
            == str(storage[index])
        )
        assert arguments[arguments.index("--maximum-backoff-seconds") + 1] == "17.0"
        assert str(configs[(index + 1) % 4]) not in arguments
    assert len({tuple(arguments) for arguments in all_arguments}) == 4
    assert "do-not-persist" not in json.dumps(manifest)
    assert manifest["genesis"]["path"] == str(
        configs[0].parents[1] / "genesis.signed.nrt"
    )
    assert migration.is_sha256(
        manifest["legacy"]["controller"]["command_sha256"]
    )


def test_root_controlled_binary_plist_carries_complete_fast_stat_seal(
    tmp_path: Path,
) -> None:
    """A trusted binary never falls back to a full read on child restart."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    binary = Path("/usr/bin/true")
    migration.require_root_controlled_executable_chain(binary)
    manifest["binary"] = migration.file_identity(
        binary, executable=True
    ).as_dict()
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
        assert arguments[arguments.index(option) + 1] == str(
            manifest["binary"][field]
        )


def test_runtime_writable_binary_is_not_eligible_for_fast_stat_seal(
    tmp_path: Path,
) -> None:
    """The migration retains full hashing when a runtime user can swap the path."""

    binary = tmp_path / "irohad"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    assert migration.binary_supports_fast_stat_seal(binary) is False


def test_manifest_rejects_store_paths_not_derived_from_sealed_root(
    tmp_path: Path,
) -> None:
    """A sealed plan cannot redirect Kura outside its adopted peer storage root."""

    manifest, _assets, _configs, _storage = render_fake_plan(tmp_path)
    manifest["peers"][0]["stores"]["kura"]["path"] = str(
        tmp_path / "redirected-kura"
    )
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

    binary = Path("/srv/taira/bin/irohad")
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

    binary = tmp_path / "irohad"
    config = tmp_path / "peer.toml"
    workdir = tmp_path / "canonical" / "taira-validator-1"
    storage = tmp_path / "storage" / "peer0"
    write_file(binary, "#!/bin/sh\nexit 0\n", 0o700)
    write_file(config, "[torii]\naddress = \"addr:127.0.0.1:29080#0000\"\n")
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
    monkeypatch.setattr(
        supervisor, "require_trusted_binary_path", lambda _path: None
    )
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
        supervisor.IdentityError, match="binary stat seal fields must be provided together"
    ):
        supervisor.require_runtime_identity(args)

    cli = [
        "--binary",
        "/tmp/irohad",
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
    write_file(config, "[torii]\naddress = \"addr:127.0.0.1:29080#0000\"\n")
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
                if stderr_path.read_text(encoding="utf-8").count(
                    "restart_in_seconds="
                ) >= 4:
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
