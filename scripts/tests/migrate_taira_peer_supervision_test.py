"""Focused tests for guarded, independent Taira validator supervision."""

from __future__ import annotations

import hashlib
import importlib.util
import json
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


def test_plan_renders_four_independent_keepalive_jobs(tmp_path: Path) -> None:
    """Each rendered LaunchDaemon owns exactly one validator and storage path."""

    manifest, assets, configs, storage = render_fake_plan(tmp_path)

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
    """The apply phase authenticates both the manifest and each staged plist."""

    manifest, assets, _configs, _storage = render_fake_plan(tmp_path)
    stage = tmp_path / "stage"
    digest = migration.write_plan(stage, manifest, assets)
    manifest_path = stage / "manifest.json"
    migration.read_manifest(manifest_path, digest)
    asset = stage / migration.SUPERVISOR_SOURCE.name
    asset.write_bytes(b"tampered")
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

    counter = tmp_path / "starts.txt"
    binary = tmp_path / "fake-irohad"
    write_file(
        binary,
        "#!/bin/sh\n"
        f"printf '%s\\n' \"$(date +%s%N)\" >> {counter}\n"
        "exit 1\n",
        0o700,
    )
    config = tmp_path / "peer.toml"
    write_file(config, "[torii]\naddress = \"addr:127.0.0.1:29080#0000\"\n")
    workdir = tmp_path / "runtime" / "peer0"
    workdir.mkdir(parents=True)
    storage = tmp_path / "storage" / "peer0"
    storage.mkdir(parents=True)
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
            str(tmp_path / "peer.pid"),
            "--initial-backoff-seconds",
            "0.02",
            "--maximum-backoff-seconds",
            "0.04",
            "--stable-uptime-seconds",
            "10",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if counter.exists() and len(counter.read_text().splitlines()) >= 4:
                break
            time.sleep(0.01)
        assert counter.exists()
        assert len(counter.read_text().splitlines()) >= 4
    finally:
        process.terminate()
        stdout, stderr = process.communicate(timeout=3)
    restart_delays = [
        float(line.rsplit("restart_in_seconds=", 1)[1])
        for line in stderr.splitlines()
        if "restart_in_seconds=" in line
    ]
    assert restart_delays[:3] == [0.02, 0.04, 0.04]
    assert max(restart_delays) <= 0.04
    assert "taira validator started" in stdout
