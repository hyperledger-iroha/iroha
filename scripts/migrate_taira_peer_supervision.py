#!/usr/bin/env python3
"""Plan or apply a guarded Taira migration to independent launchd supervision.

The ``plan`` command is read-only apart from writing a staging bundle.  It
captures the exact live legacy controller, four validator processes, binary,
configs, PID files, and existing storage-directory identities.  The ``apply``
command requires root, an explicit confirmation phrase, and the printed
manifest SHA-256.  It revalidates every captured identity before stopping the
legacy ``run-canonical.sh`` or ``launchd-run.sh`` controller and bootstrapping
four independent KeepAlive LaunchDaemons.

The migration never deletes, renames, truncates, or recreates validator
storage.  Run ``apply`` only during an announced maintenance window with no
active ledger writer.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import grp
import hashlib
import json
import math
import os
import plistlib
import pwd
import re
import shlex
import signal
import stat
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path, PurePosixPath
from typing import Any, Callable, Iterable, Sequence

PEER_COUNT = 4
SCHEMA_VERSION = 4
CONFIRMATION = "ADOPT-EXISTING-TAIRA-STORAGE"
DEFAULT_LABEL_PREFIX = "io.soramitsu.taira.validator"
SUPERVISOR_SOURCE = Path(__file__).with_name("taira_peer_supervisor.py")
CANONICAL_GENESIS_RELATIVE_PATH = Path("canonical") / "genesis.signed.nrt"
KURA_DIRECTORY_NAME = "kura"
SNAPSHOT_DIRECTORY_NAME = "snapshot"
LIFECYCLE_NODE_ID_RE = re.compile(
    r"taira-node:receipt-signer:secp256k1:sha256:[0-9a-f]{64}"
)
LIFECYCLE_VALIDATOR_IDS = tuple(
    f"taira-validator-{number}" for number in range(1, PEER_COUNT + 1)
)


class MigrationError(RuntimeError):
    """Raised when a safety or identity condition prevents migration."""


@dataclasses.dataclass(frozen=True)
class ProcessIdentity:
    """Stable process identity fields available from macOS ``ps``."""

    pid: int
    ppid: int
    started: str
    command: str
    uid: int = os.getuid()
    gid: int = os.getgid()
    cwd: str = ""

    def sealed(self) -> "SealedProcessIdentity":
        """Return a command-digest identity that cannot persist command secrets."""

        return SealedProcessIdentity(
            pid=self.pid,
            ppid=self.ppid,
            started=self.started,
            command_sha256=sha256_bytes(self.command.encode("utf-8")),
            uid=self.uid,
            gid=self.gid,
            cwd=self.cwd,
        )


@dataclasses.dataclass(frozen=True)
class SealedProcessIdentity:
    """Process identity persisted without command-line plaintext."""

    pid: int
    ppid: int
    started: str
    command_sha256: str
    uid: int
    gid: int
    cwd: str

    def as_dict(self) -> dict[str, Any]:
        """Return the redacted manifest representation."""

        return dataclasses.asdict(self)


@dataclasses.dataclass(frozen=True)
class PathIdentity:
    """Filesystem identity used to prevent path replacement during cutover."""

    path: str
    kind: str
    device: int
    inode: int
    uid: int
    gid: int
    mode: int
    size: int | None = None
    mtime_ns: int | None = None
    ctime_ns: int | None = None
    sha256: str | None = None

    def as_dict(self) -> dict[str, Any]:
        """Return a manifest-safe path representation."""

        return dataclasses.asdict(self)


ProcessInspector = Callable[[int], ProcessIdentity]


def sha256_bytes(body: bytes) -> str:
    """Return the lowercase SHA-256 digest for ``body``."""

    return hashlib.sha256(body).hexdigest()


def read_regular_file(path: Path) -> tuple[bytes, os.stat_result]:
    """Read one non-symlink regular file and reject concurrent replacement."""

    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise MigrationError(f"expected a non-symlink regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        raise MigrationError(f"file changed while it was read: {path}")
    return b"".join(chunks), after


def file_identity(path: Path, *, executable: bool = False) -> PathIdentity:
    """Capture a regular file's inode, owner, mode, size, and content digest."""

    body, info = read_regular_file(path)
    if executable and not info.st_mode & 0o111:
        raise MigrationError(f"expected executable file: {path}")
    return PathIdentity(
        path=str(path),
        kind="file",
        device=info.st_dev,
        inode=info.st_ino,
        uid=info.st_uid,
        gid=info.st_gid,
        mode=stat.S_IMODE(info.st_mode),
        size=info.st_size,
        mtime_ns=info.st_mtime_ns,
        ctime_ns=info.st_ctime_ns,
        sha256=sha256_bytes(body),
    )


def require_root_controlled_executable_chain(path: Path) -> None:
    """Require a root-owned executable path the runtime user cannot replace."""

    if not path.is_absolute() or ".." in path.parts:
        raise MigrationError(
            f"stat-sealed validator binary path is not canonical and absolute: {path}"
        )
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if stat.S_ISLNK(info.st_mode):
            raise MigrationError(
                f"stat-sealed validator binary path contains a symlink: {component}"
            )
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode) or not info.st_mode & 0o111:
                raise MigrationError(
                    "stat-sealed validator binary is not an executable regular "
                    f"file: {component}"
                )
        elif not stat.S_ISDIR(info.st_mode):
            raise MigrationError(
                f"stat-sealed validator binary ancestor is not a directory: {component}"
            )
        if info.st_uid != 0:
            raise MigrationError(
                f"stat-sealed validator binary path is not root-owned: {component}"
            )
        if stat.S_IMODE(info.st_mode) & 0o022:
            raise MigrationError(
                "stat-sealed validator binary path is group/world writable: "
                f"{component}"
            )


def binary_supports_fast_stat_seal(path: Path) -> bool:
    """Return whether repeated starts can authenticate ``path`` in O(1)."""

    try:
        require_root_controlled_executable_chain(path)
    except (MigrationError, OSError):
        return False
    return True


def directory_identity(path: Path) -> PathIdentity:
    """Capture a non-symlink directory identity without reading its contents."""

    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        raise MigrationError(f"expected a non-symlink directory: {path}")
    return PathIdentity(
        path=str(path),
        kind="directory",
        device=info.st_dev,
        inode=info.st_ino,
        uid=info.st_uid,
        gid=info.st_gid,
        mode=stat.S_IMODE(info.st_mode),
    )


def inspect_process(pid: int) -> ProcessIdentity:
    """Capture PID, parent, start time, and complete command using macOS ``ps``."""

    if pid <= 1:
        raise MigrationError(f"unsafe or invalid process id: {pid}")
    result = subprocess.run(
        [
            "/bin/ps",
            "-p",
            str(pid),
            "-o",
            "ppid=",
            "-o",
            "uid=",
            "-o",
            "gid=",
            "-o",
            "lstart=",
            "-o",
            "command=",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        raise MigrationError(f"process is not running: {pid}")
    fields = result.stdout.strip().split(maxsplit=8)
    if len(fields) != 9:
        raise MigrationError(f"could not parse process identity for pid {pid}")
    try:
        ppid = int(fields[0])
        uid = int(fields[1])
        gid = int(fields[2])
    except ValueError as exc:
        raise MigrationError(
            f"could not parse process ownership for pid {pid}"
        ) from exc
    cwd_result = subprocess.run(
        ["/usr/sbin/lsof", "-a", "-p", str(pid), "-d", "cwd", "-Fn"],
        check=False,
        capture_output=True,
        text=True,
    )
    cwd_lines = [
        line[1:]
        for line in cwd_result.stdout.splitlines()
        if line.startswith("n") and len(line) > 1
    ]
    if cwd_result.returncode != 0 or len(cwd_lines) != 1:
        raise MigrationError(f"could not inspect working directory for pid {pid}")
    cwd = cwd_lines[0]
    if not Path(cwd).is_absolute():
        raise MigrationError(f"process working directory is not absolute: pid {pid}")
    return ProcessIdentity(
        pid=pid,
        ppid=ppid,
        started=" ".join(fields[3:8]),
        command=fields[8],
        uid=uid,
        gid=gid,
        cwd=cwd,
    )


def parse_pid_file(path: Path) -> tuple[int, PathIdentity]:
    """Read one guarded PID file and return its positive integer PID."""

    body, _ = read_regular_file(path)
    try:
        text = body.decode("ascii").strip()
    except UnicodeDecodeError as exc:
        raise MigrationError(f"PID file is not ASCII: {path}") from exc
    if not text.isdecimal() or int(text) <= 1:
        raise MigrationError(f"PID file does not contain a safe PID: {path}")
    return int(text), file_identity(path)


def command_argv(command: str) -> list[str]:
    """Parse a process command while converting malformed quoting to refusal."""

    try:
        return shlex.split(command)
    except ValueError as exc:
        raise MigrationError(
            "could not parse process command "
            f"(sha256={sha256_bytes(command.encode('utf-8'))})"
        ) from exc


def require_peer_command(process: ProcessIdentity, binary: Path, config: Path) -> None:
    """Require the validator command to exactly match its planned binary/config."""

    expected = [str(binary), "--sora", "--config", str(config)]
    if command_argv(process.command) != expected:
        raise MigrationError(
            f"validator pid {process.pid} command mismatch "
            f"(sha256={sha256_bytes(process.command.encode('utf-8'))})"
        )


def require_legacy_controller_command(
    process: ProcessIdentity, base: Path, allowed_runners: Sequence[Path]
) -> Path:
    """Require a controller command tied to an approved legacy runner path."""

    for runner in allowed_runners:
        runner_text = str(runner)
        absolute_pattern = (
            rf"(?<![A-Za-z0-9_./-]){re.escape(runner_text)}" rf"(?![A-Za-z0-9_./-])"
        )
        if re.search(absolute_pattern, process.command):
            return runner
        relative_token = f"./{runner.name}"
        relative_pattern = (
            rf"(?<![A-Za-z0-9_./-]){re.escape(relative_token)}" rf"(?![A-Za-z0-9_./-])"
        )
        base_pattern = (
            rf"(?<![A-Za-z0-9_./-]){re.escape(str(base))}" rf"(?![A-Za-z0-9_./-])"
        )
        if re.search(base_pattern, process.command) and re.search(
            relative_pattern, process.command
        ):
            return runner
    allowed = ", ".join(str(path) for path in allowed_runners)
    raise MigrationError(
        f"legacy controller pid {process.pid} does not name an approved runner "
        f"({allowed}); command_sha256="
        f"{sha256_bytes(process.command.encode('utf-8'))}"
    )


def torii_port(config_path: Path) -> int:
    """Read the loopback Torii port from one rendered validator config."""

    body, _ = read_regular_file(config_path)
    try:
        config = tomllib.loads(body.decode("utf-8"))
        address = config["torii"]["address"]
    except (UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as exc:
        raise MigrationError(
            f"could not read torii.address from config: {config_path}"
        ) from exc
    if not isinstance(address, str):
        raise MigrationError(f"torii.address is not a string: {config_path}")
    host_port = address
    if host_port.startswith("addr:"):
        host_port = host_port[5:]
    host_port = host_port.split("#", 1)[0]
    try:
        port = int(host_port.rsplit(":", 1)[1])
    except (IndexError, ValueError) as exc:
        raise MigrationError(
            f"could not parse Torii port from {address!r}: {config_path}"
        ) from exc
    if not 1 <= port <= 65535:
        raise MigrationError(f"Torii port is out of range: {port}")
    return port


def normalize_absolute(path: str | Path, *, resolve: bool = False) -> Path:
    """Return an absolute path and optionally resolve its final symlink."""

    candidate = Path(path).expanduser()
    if not candidate.is_absolute():
        raise MigrationError(f"path must be absolute: {candidate}")
    return candidate.resolve(strict=True) if resolve else candidate


def require_descendant(path: Path, parent: Path, option: str) -> None:
    """Require a possibly not-yet-created path to remain below ``parent``."""

    resolved_path = path.resolve(strict=False)
    resolved_parent = parent.resolve(strict=False)
    if resolved_path == resolved_parent or not resolved_path.is_relative_to(
        resolved_parent
    ):
        raise MigrationError(f"{option} must be a child of {parent}")


def require_safe_label_prefix(value: str) -> None:
    """Reject labels that could escape the staged launchd directory."""

    if not re.fullmatch(r"[A-Za-z][A-Za-z0-9.-]{0,127}", value):
        raise MigrationError("--label-prefix is not a safe launchd label prefix")


def safe_stage_asset(stage: Path, relative: str) -> Path:
    """Resolve one manifest asset name without permitting path traversal."""

    pure = PurePosixPath(relative)
    if (
        pure.is_absolute()
        or not pure.parts
        or any(part in ("", ".", "..") for part in pure.parts)
    ):
        raise MigrationError(f"unsafe staged asset path: {relative!r}")
    target = stage.joinpath(*pure.parts)
    if not target.resolve(strict=False).is_relative_to(stage.resolve(strict=True)):
        raise MigrationError(f"staged asset escapes bundle: {relative!r}")
    return target


def default_peer_paths(base: Path, kind: str) -> list[Path]:
    """Return the four paths for the known shared-host canonical layout."""

    if kind == "configs":
        return [
            base / "canonical" / f"taira-validator-{index + 1}" / "config.toml"
            for index in range(PEER_COUNT)
        ]
    if kind == "storage":
        return [base / "storage" / f"peer{index}" for index in range(PEER_COUNT)]
    if kind == "pids":
        return [base / f"canonical-peer{index}.pid" for index in range(PEER_COUNT)]
    raise AssertionError(f"unsupported peer path kind: {kind}")


def canonical_genesis_path(base: Path) -> Path:
    """Return the one signed genesis file used by the canonical rollout."""

    return base / CANONICAL_GENESIS_RELATIVE_PATH


def peer_store_paths(storage_root: Path) -> tuple[Path, Path]:
    """Derive the Kura and snapshot directories owned by one sealed peer root."""

    return (
        storage_root / KURA_DIRECTORY_NAME,
        storage_root / SNAPSHOT_DIRECTORY_NAME,
    )


def require_exact_peer_paths(
    values: Sequence[str] | None, defaults: Sequence[Path], option: str
) -> list[Path]:
    """Normalize a repeated four-peer option or use its layout defaults."""

    if not values:
        return list(defaults)
    if len(values) != PEER_COUNT:
        raise MigrationError(f"{option} must be supplied exactly {PEER_COUNT} times")
    return [normalize_absolute(value) for value in values]


def require_authenticated_node_bindings(
    values: Sequence[str] | None,
) -> tuple[str, ...]:
    """Return exact deploy-authenticated node IDs in canonical peer order."""

    if values is None or len(values) != PEER_COUNT:
        raise MigrationError(
            "--authenticated-node-binding must be supplied exactly once for each "
            "taira-validator-1..4 slug"
        )
    node_ids_by_validator: dict[str, str] = {}
    for value in values:
        validator_id, separator, node_id = value.partition("=")
        if not separator or validator_id not in LIFECYCLE_VALIDATOR_IDS:
            raise MigrationError(
                "authenticated node binding must be "
                "taira-validator-N=<canonical-receipt-signer-node-id>"
            )
        if validator_id in node_ids_by_validator:
            raise MigrationError(
                f"authenticated node binding repeats validator slug: {validator_id}"
            )
        if LIFECYCLE_NODE_ID_RE.fullmatch(node_id) is None:
            raise MigrationError("authenticated lifecycle node ID is not canonical")
        node_ids_by_validator[validator_id] = node_id
    if set(node_ids_by_validator) != set(LIFECYCLE_VALIDATOR_IDS):
        raise MigrationError(
            "authenticated node bindings must cover taira-validator-1..4 exactly"
        )
    node_ids = tuple(
        node_ids_by_validator[validator_id]
        for validator_id in LIFECYCLE_VALIDATOR_IDS
    )
    if len(set(node_ids)) != PEER_COUNT:
        raise MigrationError("authenticated lifecycle node IDs must be distinct")
    return node_ids


def launchd_plist(
    *,
    peer: dict[str, Any],
    manifest: dict[str, Any],
    installed_supervisor: Path,
    python_path: Path,
) -> bytes:
    """Render one independent, lifecycle-journaled validator LaunchDaemon plist."""

    runtime = manifest["runtime"]
    working_directory = peer["working_directory"]
    storage = peer["storage"]
    log_path = Path(manifest["install"]["logs_dir"]) / (
        f"validator-{peer['number']}-supervisor.log"
    )
    program_arguments = [
        str(python_path),
        str(installed_supervisor),
        "--binary",
        manifest["binary"]["path"],
        "--binary-sha256",
        manifest["binary"]["sha256"],
        "--config",
        peer["config"]["path"],
        "--config-sha256",
        peer["config"]["sha256"],
        "--workdir",
        working_directory["path"],
        "--workdir-device",
        str(working_directory["device"]),
        "--workdir-inode",
        str(working_directory["inode"]),
        "--storage-dir",
        storage["path"],
        "--storage-device",
        str(storage["device"]),
        "--storage-inode",
        str(storage["inode"]),
        "--pid-file",
        peer["supervised_pid_file"],
        "--terminal-unhealthy-file",
        peer["terminal_unhealthy_file"],
        "--restart-generation",
        runtime["restart_generation"],
        "--initial-backoff-seconds",
        str(runtime["initial_backoff_seconds"]),
        "--maximum-backoff-seconds",
        str(runtime["maximum_backoff_seconds"]),
        "--stable-uptime-seconds",
        str(runtime["stable_uptime_seconds"]),
        "--rapid-fatal-uptime-seconds",
        str(runtime["rapid_fatal_uptime_seconds"]),
    ]
    lifecycle = peer.get("lifecycle")
    validator_id = f"taira-validator-{peer['number']}"
    expected_root = (
        Path(manifest["install"]["directory"]) / "lifecycle" / validator_id
    )
    if (
        not isinstance(lifecycle, dict)
        or set(lifecycle) != {"journal_root", "node_id", "validator_id"}
        or lifecycle.get("validator_id") != validator_id
        or lifecycle.get("journal_root") != str(expected_root)
        or not expected_root.is_absolute()
        or ".." in expected_root.parts
        or not isinstance(lifecycle.get("node_id"), str)
        or LIFECYCLE_NODE_ID_RE.fullmatch(lifecycle["node_id"]) is None
    ):
        raise MigrationError(
            "lifecycle journal path or authenticated node ID is not canonical"
        )
    program_arguments.extend(
        [
            "--lifecycle-journal-root",
            str(expected_root),
            "--validator-id",
            validator_id,
            "--node-id",
            lifecycle["node_id"],
        ]
    )
    if runtime["binary_stat_sealed"]:
        binary = manifest["binary"]
        program_arguments[6:6] = [
            "--binary-device",
            str(binary["device"]),
            "--binary-inode",
            str(binary["inode"]),
            "--binary-size",
            str(binary["size"]),
            "--binary-mtime-ns",
            str(binary["mtime_ns"]),
            "--binary-ctime-ns",
            str(binary["ctime_ns"]),
        ]
    payload = {
        "Label": peer["label"],
        "ProgramArguments": program_arguments,
        "WorkingDirectory": working_directory["path"],
        "RunAtLoad": True,
        "KeepAlive": True,
        "ThrottleInterval": runtime["launchd_throttle_seconds"],
        "ProcessType": "Background",
        "ExitTimeOut": runtime["termination_timeout_seconds"],
        "AbandonProcessGroup": False,
        "UserName": manifest["runtime_user"]["name"],
        "GroupName": manifest["runtime_user"]["group"],
        "EnvironmentVariables": {
            "GENESIS": manifest["genesis"]["path"],
            "KURA_STORE_DIR": peer["stores"]["kura"]["path"],
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "RUST_LOG": runtime["rust_log"],
            "SNAPSHOT_STORE_DIR": peer["stores"]["snapshot"]["path"],
            "ZK_HALO2_ENABLED": runtime["zk_halo2_enabled"],
        },
        "StandardOutPath": str(log_path),
        "StandardErrorPath": str(log_path),
    }
    return plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)


def create_plan(
    args: argparse.Namespace, *, process_inspector: ProcessInspector = inspect_process
) -> tuple[dict[str, Any], dict[str, bytes]]:
    """Inspect the live legacy topology and render a non-mutating staged bundle."""

    require_safe_label_prefix(args.label_prefix)
    base = normalize_absolute(args.base)
    base_identity = directory_identity(base)
    genesis = canonical_genesis_path(base)
    require_descendant(genesis, base, "canonical genesis")
    genesis_identity = file_identity(genesis)
    binary = normalize_absolute(args.irohad or base / "bin" / "iroha3d", resolve=True)
    binary_stat_sealed = binary_supports_fast_stat_seal(binary)
    if not binary_stat_sealed:
        require_descendant(binary, base, "--irohad")
    binary_identity = file_identity(binary, executable=True)
    python_path = normalize_absolute(args.python, resolve=True)
    python_identity = file_identity(python_path, executable=True)
    supervisor_identity = file_identity(SUPERVISOR_SOURCE, executable=False)
    configs = require_exact_peer_paths(
        args.config, default_peer_paths(base, "configs"), "--config"
    )
    storage_dirs = require_exact_peer_paths(
        args.storage, default_peer_paths(base, "storage"), "--storage"
    )
    pid_files = require_exact_peer_paths(
        args.pid_file, default_peer_paths(base, "pids"), "--pid-file"
    )
    authenticated_node_ids = require_authenticated_node_bindings(
        args.authenticated_node_binding
    )
    allowed_runners = [
        normalize_absolute(path)
        for path in (
            args.legacy_runner or [base / "run-canonical.sh", base / "launchd-run.sh"]
        )
    ]
    for runner in allowed_runners:
        require_descendant(runner, base, "--legacy-runner")
    existing_runners = [path for path in allowed_runners if path.exists()]
    if not existing_runners:
        raise MigrationError("none of the approved legacy runner paths exists")
    runner_identities = [
        file_identity(path, executable=True) for path in existing_runners
    ]

    runtime_uid = base_identity.uid
    runtime_gid = base_identity.gid
    if runtime_uid == 0:
        raise MigrationError("refusing to install validators that would run as root")
    if genesis_identity.uid != runtime_uid or genesis_identity.gid != runtime_gid:
        raise MigrationError("canonical genesis owner differs from deployment owner")
    try:
        runtime_user = pwd.getpwuid(runtime_uid).pw_name
        runtime_group = grp.getgrgid(runtime_gid).gr_name
    except KeyError as exc:
        raise MigrationError("could not resolve deployment owner user/group") from exc

    install_dir = normalize_absolute(args.install_dir or base / "supervision")
    logs_dir = normalize_absolute(args.logs_dir or install_dir / "logs")
    require_descendant(install_dir, base, "--install-dir")
    require_descendant(logs_dir, install_dir, "--logs-dir")
    launch_daemons_dir = normalize_absolute(args.launch_daemons_dir)
    installed_supervisor = install_dir / SUPERVISOR_SOURCE.name
    peers: list[dict[str, Any]] = []
    parent_pids: set[int] = set()
    seen_ports: set[int] = set()
    for index, (config, storage_dir, pid_file, authenticated_node_id) in enumerate(
        zip(
            configs,
            storage_dirs,
            pid_files,
            authenticated_node_ids,
            strict=True,
        )
    ):
        require_descendant(config, base, "--config")
        require_descendant(storage_dir, base, "--storage")
        require_descendant(pid_file, base, "--pid-file")
        config_identity = file_identity(config)
        storage_identity = directory_identity(storage_dir)
        kura_dir, snapshot_dir = peer_store_paths(storage_dir)
        kura_identity = directory_identity(kura_dir)
        snapshot_identity = directory_identity(snapshot_dir)
        if (
            config_identity.uid != runtime_uid
            or config_identity.gid != runtime_gid
            or storage_identity.uid != runtime_uid
            or storage_identity.gid != runtime_gid
            or kura_identity.uid != runtime_uid
            or kura_identity.gid != runtime_gid
            or snapshot_identity.uid != runtime_uid
            or snapshot_identity.gid != runtime_gid
        ):
            raise MigrationError(
                f"peer {index + 1} config/storage owner differs from deployment owner"
            )
        pid, pid_identity = parse_pid_file(pid_file)
        if pid_identity.uid != runtime_uid or pid_identity.gid != runtime_gid:
            raise MigrationError(
                f"peer {index + 1} PID file owner differs from deployment owner"
            )
        process = process_inspector(pid)
        require_peer_command(process, binary, config)
        if process.uid != runtime_uid or process.gid != runtime_gid:
            raise MigrationError(
                f"peer {index + 1} process owner differs from deployment owner"
            )
        working_directory_path = normalize_absolute(process.cwd)
        require_descendant(
            working_directory_path, base, "legacy validator working directory"
        )
        working_directory_identity = directory_identity(working_directory_path)
        if (
            working_directory_identity.uid != runtime_uid
            or working_directory_identity.gid != runtime_gid
        ):
            raise MigrationError(
                f"peer {index + 1} working-directory owner differs from deployment owner"
            )
        parent_pids.add(process.ppid)
        port = torii_port(config)
        if port in seen_ports:
            raise MigrationError(f"duplicate Torii port across peers: {port}")
        seen_ports.add(port)
        peers.append(
            {
                "index": index,
                "number": index + 1,
                "label": f"{args.label_prefix}-{index + 1}",
                "legacy_process": process.sealed().as_dict(),
                "legacy_pid_file": pid_identity.as_dict(),
                "config": config_identity.as_dict(),
                "working_directory": working_directory_identity.as_dict(),
                "storage": storage_identity.as_dict(),
                "stores": {
                    "kura": kura_identity.as_dict(),
                    "snapshot": snapshot_identity.as_dict(),
                },
                "torii_port": port,
                "supervised_pid_file": str(
                    install_dir / "pids" / f"validator-{index + 1}.pid"
                ),
                "terminal_unhealthy_file": str(
                    install_dir
                    / "terminal"
                    / f"validator-{index + 1}-terminal-unhealthy.json"
                ),
                "lifecycle": {
                    "journal_root": str(
                        lifecycle_journal_root(install_dir, index + 1)
                    ),
                    "node_id": authenticated_node_id,
                    "validator_id": f"taira-validator-{index + 1}",
                },
            }
        )
    if len(parent_pids) != 1:
        raise MigrationError(
            "legacy validators do not share exactly one controller parent process"
        )
    controller = process_inspector(parent_pids.pop())
    selected_runner = require_legacy_controller_command(
        controller, base, existing_runners
    )
    if controller.uid != runtime_uid or controller.gid != runtime_gid:
        raise MigrationError("legacy controller owner differs from deployment owner")

    if (
        not math.isfinite(args.initial_backoff_seconds)
        or args.initial_backoff_seconds <= 0
    ):
        raise MigrationError("--initial-backoff-seconds must be positive")
    if (
        not math.isfinite(args.maximum_backoff_seconds)
        or args.maximum_backoff_seconds < args.initial_backoff_seconds
    ):
        raise MigrationError(
            "--maximum-backoff-seconds must be at least the initial backoff"
        )
    if not math.isfinite(args.stable_uptime_seconds) or args.stable_uptime_seconds <= 0:
        raise MigrationError("--stable-uptime-seconds must be positive")
    if args.launchd_throttle_seconds <= 0:
        raise MigrationError("--launchd-throttle-seconds must be positive")
    if args.termination_timeout_seconds <= 0:
        raise MigrationError("--termination-timeout-seconds must be positive")
    if args.health_timeout_seconds <= 0:
        raise MigrationError("--health-timeout-seconds must be positive")
    if (
        not math.isfinite(args.rapid_fatal_uptime_seconds)
        or args.rapid_fatal_uptime_seconds <= 0
    ):
        raise MigrationError("--rapid-fatal-uptime-seconds must be positive")
    if args.restart_generation is not None and not is_sha256(args.restart_generation):
        raise MigrationError(
            "--restart-generation must be one lowercase SHA-256 digest"
        )
    restart_generation = (
        args.restart_generation
        or hashlib.sha256(
            json.dumps(
                {
                    "binary": binary_identity.sha256,
                    "configs": [peer["config"]["sha256"] for peer in peers],
                    "schema": "taira-supervision-restart-generation-v1",
                },
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            ).encode("ascii")
        ).hexdigest()
    )

    manifest: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "label_prefix": args.label_prefix,
        "base": base_identity.as_dict(),
        "genesis": genesis_identity.as_dict(),
        "binary": binary_identity.as_dict(),
        "python": python_identity.as_dict(),
        "supervisor_source": supervisor_identity.as_dict(),
        "runtime_user": {
            "name": runtime_user,
            "group": runtime_group,
            "uid": runtime_uid,
            "gid": runtime_gid,
        },
        "legacy": {
            "controller": controller.sealed().as_dict(),
            "selected_runner": str(selected_runner),
            "runners": [identity.as_dict() for identity in runner_identities],
        },
        "runtime": {
            "initial_backoff_seconds": args.initial_backoff_seconds,
            "maximum_backoff_seconds": args.maximum_backoff_seconds,
            "stable_uptime_seconds": args.stable_uptime_seconds,
            "rapid_fatal_uptime_seconds": args.rapid_fatal_uptime_seconds,
            "restart_generation": restart_generation,
            "launchd_throttle_seconds": args.launchd_throttle_seconds,
            "termination_timeout_seconds": args.termination_timeout_seconds,
            "health_timeout_seconds": args.health_timeout_seconds,
            "binary_stat_sealed": binary_stat_sealed,
            "rust_log": args.rust_log,
            "zk_halo2_enabled": args.zk_halo2_enabled,
        },
        "install": {
            "directory": str(install_dir),
            "logs_dir": str(logs_dir),
            "supervisor": str(installed_supervisor),
            "launch_daemons_dir": str(launch_daemons_dir),
        },
        "peers": peers,
    }
    supervisor_body, _ = read_regular_file(SUPERVISOR_SOURCE)
    assets: dict[str, bytes] = {SUPERVISOR_SOURCE.name: supervisor_body}
    for peer in peers:
        relative = f"launchd/{peer['label']}.plist"
        assets[relative] = launchd_plist(
            peer=peer,
            manifest=manifest,
            installed_supervisor=installed_supervisor,
            python_path=python_path,
        )
    manifest["assets"] = {
        name: {"sha256": sha256_bytes(body), "size": len(body)}
        for name, body in sorted(assets.items())
    }
    return manifest, assets


def atomic_write(path: Path, body: bytes, mode: int) -> None:
    """Write a file atomically without following an existing symlink."""

    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        raise MigrationError(f"refusing to replace staged path: {path}")
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        mode,
    )
    try:
        offset = 0
        while offset < len(body):
            offset += os.write(descriptor, body[offset:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, path)
    path.chmod(mode)


def write_plan(
    output_dir: Path, manifest: dict[str, Any], assets: dict[str, bytes]
) -> str:
    """Write a new immutable-by-convention staging directory and return its digest."""

    if output_dir.exists() or output_dir.is_symlink():
        raise MigrationError(f"output directory already exists: {output_dir}")
    output_dir.mkdir(mode=0o700, parents=True)
    for relative, body in assets.items():
        mode = 0o700 if relative == SUPERVISOR_SOURCE.name else 0o600
        atomic_write(safe_stage_asset(output_dir, relative), body, mode)
    manifest_body = (
        json.dumps(manifest, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")
    atomic_write(output_dir / "manifest.json", manifest_body, 0o600)
    return sha256_bytes(manifest_body)


def dict_to_path_identity(payload: dict[str, Any]) -> PathIdentity:
    """Decode one manifest path identity."""

    return PathIdentity(**payload)


def dict_to_sealed_process_identity(
    payload: dict[str, Any],
) -> SealedProcessIdentity:
    """Decode one redacted manifest process identity."""

    return SealedProcessIdentity(**payload)


def require_path_unchanged(payload: dict[str, Any]) -> None:
    """Re-capture a path and require exact planned identity."""

    expected = dict_to_path_identity(payload)
    path = Path(expected.path)
    actual = (
        file_identity(path, executable=False)
        if expected.kind == "file"
        else directory_identity(path)
    )
    if actual != expected:
        raise MigrationError(f"planned path identity changed: {path}")


def require_process_unchanged(
    payload: dict[str, Any], *, process_inspector: ProcessInspector = inspect_process
) -> None:
    """Re-capture a process and require exact PID/start/parent/command identity."""

    expected = dict_to_sealed_process_identity(payload)
    actual = process_inspector(expected.pid)
    if actual.sealed() != expected:
        raise MigrationError(f"planned process identity changed: pid {expected.pid}")


def is_sha256(value: object) -> bool:
    """Return whether ``value`` is one canonical lowercase SHA-256 digest."""

    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def validate_manifest_path_identity(
    payload: object, *, expected_kind: str
) -> PathIdentity:
    """Decode and validate one sealed file or directory identity."""

    if not isinstance(payload, dict):
        raise MigrationError("manifest path identity must be an object")
    try:
        identity = dict_to_path_identity(payload)
    except (TypeError, ValueError) as exc:
        raise MigrationError("manifest path identity is invalid") from exc
    if identity.kind != expected_kind or not Path(identity.path).is_absolute():
        raise MigrationError(f"manifest {expected_kind} identity is inconsistent")
    if (
        not isinstance(identity.device, int)
        or not isinstance(identity.inode, int)
        or identity.inode <= 0
        or not isinstance(identity.uid, int)
        or identity.uid < 0
        or not isinstance(identity.gid, int)
        or identity.gid < 0
        or not isinstance(identity.mode, int)
        or not 0 <= identity.mode <= 0o7777
    ):
        raise MigrationError(f"manifest {expected_kind} metadata is invalid")
    if expected_kind == "file":
        if (
            not isinstance(identity.size, int)
            or identity.size < 0
            or not isinstance(identity.mtime_ns, int)
            or identity.mtime_ns < 0
            or not isinstance(identity.ctime_ns, int)
            or identity.ctime_ns < 0
            or not is_sha256(identity.sha256)
        ):
            raise MigrationError("manifest file digest/stat seal is invalid")
    elif (
        identity.size is not None
        or identity.mtime_ns is not None
        or identity.ctime_ns is not None
        or identity.sha256 is not None
    ):
        raise MigrationError("manifest directory unexpectedly carries file metadata")
    return identity


def validate_manifest_process_identity(payload: object) -> SealedProcessIdentity:
    """Decode and validate one command-redacted process identity."""

    if not isinstance(payload, dict):
        raise MigrationError("manifest process identity must be an object")
    try:
        identity = dict_to_sealed_process_identity(payload)
    except (TypeError, ValueError) as exc:
        raise MigrationError("manifest process identity is invalid") from exc
    if (
        not isinstance(identity.pid, int)
        or identity.pid <= 1
        or not isinstance(identity.ppid, int)
        or identity.ppid < 0
        or not isinstance(identity.started, str)
        or not identity.started
        or not is_sha256(identity.command_sha256)
        or not isinstance(identity.uid, int)
        or identity.uid < 0
        or not isinstance(identity.gid, int)
        or identity.gid < 0
        or not isinstance(identity.cwd, str)
        or not Path(identity.cwd).is_absolute()
    ):
        raise MigrationError("manifest process identity metadata is invalid")
    return identity


def validate_manifest_shape(manifest: object) -> dict[str, Any]:
    """Validate the sealed first-release four-peer manifest contract."""

    if not isinstance(manifest, dict):
        raise MigrationError("manifest root must be an object")
    try:
        if manifest["schema_version"] != SCHEMA_VERSION:
            raise MigrationError("unsupported supervision manifest schema")
        label_prefix = manifest["label_prefix"]
        require_safe_label_prefix(label_prefix)
        peers = manifest["peers"]
        if not isinstance(peers, list) or len(peers) != PEER_COUNT:
            raise MigrationError(f"manifest must contain exactly {PEER_COUNT} peers")
        base_identity = validate_manifest_path_identity(
            manifest["base"], expected_kind="directory"
        )
        base = Path(base_identity.path)
        genesis_identity = validate_manifest_path_identity(
            manifest["genesis"], expected_kind="file"
        )
        if Path(genesis_identity.path) != canonical_genesis_path(base):
            raise MigrationError("manifest canonical genesis path is inconsistent")
        install_dir = Path(manifest["install"]["directory"])
        logs_dir = Path(manifest["install"]["logs_dir"])
        supervisor = Path(manifest["install"]["supervisor"])
        launch_daemons_dir = Path(manifest["install"]["launch_daemons_dir"])
        for path in (base, install_dir, logs_dir, supervisor, launch_daemons_dir):
            if not path.is_absolute():
                raise MigrationError(f"manifest path must be absolute: {path}")
        require_descendant(install_dir, base, "manifest install directory")
        require_descendant(logs_dir, install_dir, "manifest logs directory")
        if supervisor != install_dir / SUPERVISOR_SOURCE.name:
            raise MigrationError("manifest supervisor install path is inconsistent")
        runtime_uid = int(manifest["runtime_user"]["uid"])
        runtime_gid = int(manifest["runtime_user"]["gid"])
        if runtime_uid <= 0 or runtime_gid < 0:
            raise MigrationError("manifest would run validators as root")
        if base_identity.uid != runtime_uid or base_identity.gid != runtime_gid:
            raise MigrationError("manifest base owner differs from runtime owner")
        if genesis_identity.uid != runtime_uid or genesis_identity.gid != runtime_gid:
            raise MigrationError("manifest genesis owner differs from runtime owner")
        if (
            not isinstance(manifest["runtime_user"]["name"], str)
            or not manifest["runtime_user"]["name"]
            or not isinstance(manifest["runtime_user"]["group"], str)
            or not manifest["runtime_user"]["group"]
        ):
            raise MigrationError("manifest runtime user/group names are invalid")

        expected_assets = {SUPERVISOR_SOURCE.name}
        peer_pids: set[int] = set()
        storage_identities: set[tuple[str, int, int]] = set()
        config_paths: set[str] = set()
        lifecycle_roots: set[str] = set()
        lifecycle_node_ids: set[str] = set()
        ports: set[int] = set()
        controller = validate_manifest_process_identity(
            manifest["legacy"]["controller"]
        )
        controller_pid = int(controller.pid)
        if (
            controller_pid <= 1
            or controller.uid != runtime_uid
            or controller.gid != runtime_gid
        ):
            raise MigrationError("manifest legacy controller identity is invalid")
        for index, peer in enumerate(peers):
            number = index + 1
            expected_label = f"{label_prefix}-{number}"
            if (
                peer["index"] != index
                or peer["number"] != number
                or peer["label"] != expected_label
            ):
                raise MigrationError("manifest peer ordering or label is inconsistent")
            expected_assets.add(f"launchd/{expected_label}.plist")
            expected_pid_file = install_dir / "pids" / f"validator-{number}.pid"
            if Path(peer["supervised_pid_file"]) != expected_pid_file:
                raise MigrationError("manifest supervised PID path is inconsistent")
            expected_terminal_file = (
                install_dir / "terminal" / f"validator-{number}-terminal-unhealthy.json"
            )
            if Path(peer["terminal_unhealthy_file"]) != expected_terminal_file:
                raise MigrationError("manifest terminal-unhealthy path is inconsistent")
            lifecycle = peer["lifecycle"]
            expected_validator_id = f"taira-validator-{number}"
            expected_lifecycle_root = lifecycle_journal_root(install_dir, number)
            if (
                not isinstance(lifecycle, dict)
                or set(lifecycle) != {"journal_root", "node_id", "validator_id"}
                or lifecycle.get("validator_id") != expected_validator_id
                or lifecycle.get("journal_root") != str(expected_lifecycle_root)
                or not isinstance(lifecycle.get("node_id"), str)
                or LIFECYCLE_NODE_ID_RE.fullmatch(lifecycle["node_id"]) is None
            ):
                raise MigrationError("manifest lifecycle identity is inconsistent")
            if lifecycle["journal_root"] in lifecycle_roots:
                raise MigrationError("manifest contains duplicate lifecycle root")
            if lifecycle["node_id"] in lifecycle_node_ids:
                raise MigrationError("manifest contains duplicate lifecycle node ID")
            lifecycle_roots.add(lifecycle["journal_root"])
            lifecycle_node_ids.add(lifecycle["node_id"])
            legacy_process = validate_manifest_process_identity(peer["legacy_process"])
            working_directory = validate_manifest_path_identity(
                peer["working_directory"], expected_kind="directory"
            )
            if (
                legacy_process.pid <= 1
                or legacy_process.ppid != controller_pid
                or legacy_process.uid != runtime_uid
                or legacy_process.gid != runtime_gid
                or Path(legacy_process.cwd).resolve(strict=False)
                != Path(working_directory.path).resolve(strict=False)
            ):
                raise MigrationError(
                    "manifest peer process/cwd/controller identity is inconsistent"
                )
            if legacy_process.pid in peer_pids:
                raise MigrationError("manifest contains duplicate validator PIDs")
            peer_pids.add(legacy_process.pid)
            pid_identity = validate_manifest_path_identity(
                peer["legacy_pid_file"], expected_kind="file"
            )
            config_identity = validate_manifest_path_identity(
                peer["config"], expected_kind="file"
            )
            storage = validate_manifest_path_identity(
                peer["storage"], expected_kind="directory"
            )
            stores = peer["stores"]
            if not isinstance(stores, dict) or set(stores) != {
                "kura",
                "snapshot",
            }:
                raise MigrationError("manifest peer stores are inconsistent")
            kura = validate_manifest_path_identity(
                stores["kura"], expected_kind="directory"
            )
            snapshot = validate_manifest_path_identity(
                stores["snapshot"], expected_kind="directory"
            )
            expected_kura, expected_snapshot = peer_store_paths(Path(storage.path))
            if (
                Path(kura.path) != expected_kura
                or Path(snapshot.path) != expected_snapshot
            ):
                raise MigrationError(
                    "manifest peer store paths are inconsistent with storage root"
                )
            for identity in (
                pid_identity,
                config_identity,
                working_directory,
                storage,
                kura,
                snapshot,
            ):
                if identity.uid != runtime_uid or identity.gid != runtime_gid:
                    raise MigrationError(
                        "manifest peer path owner differs from runtime owner"
                    )
                require_descendant(
                    Path(identity.path), base, "manifest peer runtime path"
                )
            storage_key = (storage.path, storage.device, storage.inode)
            if storage_key in storage_identities:
                raise MigrationError("manifest contains duplicate storage identity")
            storage_identities.add(storage_key)
            config_path = config_identity.path
            if config_path in config_paths:
                raise MigrationError("manifest contains duplicate config path")
            config_paths.add(config_path)
            port = int(peer["torii_port"])
            if not 1 <= port <= 65535 or port in ports:
                raise MigrationError(
                    "manifest contains invalid or duplicate Torii port"
                )
            ports.add(port)

        assets = manifest["assets"]
        if not isinstance(assets, dict) or set(assets) != expected_assets:
            raise MigrationError("manifest asset set does not match its four peers")
        for relative, identity in assets.items():
            safe_stage_asset(Path.cwd(), relative)
            if (
                not isinstance(identity, dict)
                or not is_sha256(identity.get("sha256"))
                or not isinstance(identity.get("size"), int)
                or identity["size"] < 0
            ):
                raise MigrationError(f"invalid manifest asset identity: {relative}")
        if manifest["legacy"]["selected_runner"] not in {
            runner["path"] for runner in manifest["legacy"]["runners"]
        }:
            raise MigrationError("selected legacy runner is not sealed in the manifest")
        for runner in manifest["legacy"]["runners"]:
            runner_identity = validate_manifest_path_identity(
                runner, expected_kind="file"
            )
            require_descendant(
                Path(runner_identity.path), base, "manifest legacy runner"
            )
        for key in ("binary", "python", "supervisor_source"):
            identity = validate_manifest_path_identity(
                manifest[key], expected_kind="file"
            )
            if key == "binary":
                binary_stat_sealed = manifest["runtime"]["binary_stat_sealed"]
                if not isinstance(binary_stat_sealed, bool):
                    raise MigrationError("manifest binary stat-seal policy is invalid")
                if binary_stat_sealed:
                    require_root_controlled_executable_chain(Path(identity.path))
                else:
                    require_descendant(
                        Path(identity.path), base, "manifest validator binary"
                    )
        runtime = manifest["runtime"]
        if (
            not is_sha256(runtime["restart_generation"])
            or not isinstance(runtime["rapid_fatal_uptime_seconds"], (int, float))
            or isinstance(runtime["rapid_fatal_uptime_seconds"], bool)
            or not math.isfinite(runtime["rapid_fatal_uptime_seconds"])
            or runtime["rapid_fatal_uptime_seconds"] <= 0
        ):
            raise MigrationError("manifest fatal-loop policy is invalid")
    except (KeyError, TypeError, ValueError) as exc:
        raise MigrationError("manifest structure is invalid") from exc
    return manifest


def read_manifest(
    path: Path, expected_sha256: str
) -> tuple[dict[str, Any], Path, dict[str, bytes]]:
    """Read and authenticate a staged manifest plus all rendered assets."""

    if not is_sha256(expected_sha256):
        raise MigrationError("expected manifest digest must be lowercase SHA-256")
    body, _ = read_regular_file(path)
    actual_sha256 = sha256_bytes(body)
    if actual_sha256 != expected_sha256:
        raise MigrationError(
            f"manifest digest mismatch: expected {expected_sha256}, got {actual_sha256}"
        )
    try:
        manifest = json.loads(body)
    except json.JSONDecodeError as exc:
        raise MigrationError(f"invalid manifest JSON: {path}") from exc
    manifest = validate_manifest_shape(manifest)
    stage = path.parent
    authenticated_assets: dict[str, bytes] = {}
    for relative, expected in manifest["assets"].items():
        asset_body, _ = read_regular_file(safe_stage_asset(stage, relative))
        if (
            len(asset_body) != expected["size"]
            or sha256_bytes(asset_body) != expected["sha256"]
        ):
            raise MigrationError(f"staged asset identity changed: {relative}")
        authenticated_assets[relative] = asset_body
    return manifest, stage, authenticated_assets


def run_checked(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
    """Run one operator command and surface stdout/stderr on refusal."""

    result = subprocess.run(command, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no output"
        raise MigrationError(f"command failed ({shlex.join(command)}): {detail}")
    return result


def ensure_install_directory(path: Path, *, uid: int, gid: int, mode: int) -> None:
    """Create or validate an owned, non-symlink installation directory."""

    if path.exists() or path.is_symlink():
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
            raise MigrationError(f"unsafe installation directory: {path}")
        if info.st_uid != uid or info.st_gid != gid:
            raise MigrationError(f"installation directory owner changed: {path}")
    else:
        path.mkdir(mode=mode, parents=True)
        os.chown(path, uid, gid)
    path.chmod(mode)


def lifecycle_journal_root(install_dir: Path, peer_number: int) -> Path:
    """Return the fixed lifecycle journal root for one migrated peer."""

    if peer_number not in range(1, PEER_COUNT + 1):
        raise MigrationError("lifecycle peer number is outside the four-peer cohort")
    return install_dir / "lifecycle" / f"taira-validator-{peer_number}"


def ensure_lifecycle_journal_layout(
    install_dir: Path, *, uid: int, gid: int
) -> tuple[Path, ...]:
    """Provision four distinct mode-0700 owner-private journal roots."""

    parent = install_dir / "lifecycle"
    ensure_install_directory(parent, uid=uid, gid=gid, mode=0o700)
    roots = tuple(
        lifecycle_journal_root(install_dir, number)
        for number in range(1, PEER_COUNT + 1)
    )
    if len(set(roots)) != PEER_COUNT:
        raise MigrationError("lifecycle journal roots are not distinct")
    for root in roots:
        ensure_install_directory(root, uid=uid, gid=gid, mode=0o700)
        info = root.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != uid
            or info.st_gid != gid
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            raise MigrationError(f"unsafe lifecycle journal root: {root}")
    return roots


def copy_new_file(
    body: bytes, destination: Path, *, uid: int, gid: int, mode: int
) -> None:
    """Install authenticated bytes and refuse overwrite or symlink traversal."""

    if destination.exists() or destination.is_symlink():
        raise MigrationError(f"refusing to replace installed file: {destination}")
    atomic_write(destination, body, mode)
    os.chown(destination, uid, gid)


def wait_processes_gone(
    processes: Iterable[ProcessIdentity | SealedProcessIdentity], timeout: int
) -> bool:
    """Wait until all exact planned PIDs have exited."""

    deadline = time.monotonic() + timeout
    pids = [process.pid for process in processes]
    while time.monotonic() < deadline:
        if all(not process_is_alive(pid) for pid in pids):
            return True
        time.sleep(0.25)
    return all(not process_is_alive(pid) for pid in pids)


def process_is_alive(pid: int) -> bool:
    """Return whether a PID exists and is not a zombie."""

    result = subprocess.run(
        ["/bin/ps", "-p", str(pid), "-o", "state="],
        check=False,
        capture_output=True,
        text=True,
    )
    state = result.stdout.strip()
    return result.returncode == 0 and bool(state) and not state.startswith("Z")


def endpoint_healthy(port: int) -> bool:
    """Probe one loopback Torii health endpoint with a short timeout."""

    try:
        with urllib.request.urlopen(
            f"http://127.0.0.1:{port}/health", timeout=2
        ) as response:
            return 200 <= response.status < 300
    except (OSError, urllib.error.URLError):
        return False


def wait_healthy(peers: Sequence[dict[str, Any]], timeout: int) -> list[int]:
    """Wait for all peer health endpoints and return any ports still unhealthy."""

    deadline = time.monotonic() + timeout
    pending = {int(peer["torii_port"]) for peer in peers}
    while pending and time.monotonic() < deadline:
        pending = {port for port in pending if not endpoint_healthy(port)}
        if pending:
            time.sleep(1)
    return sorted(pending)


def inspect_supervised_peers_once(
    manifest: dict[str, Any],
    *,
    process_inspector: ProcessInspector = inspect_process,
) -> list[ProcessIdentity]:
    """Require each new PID file to name its exact one-peer validator command."""

    binary = Path(manifest["binary"]["path"])
    runtime_uid = int(manifest["runtime_user"]["uid"])
    runtime_gid = int(manifest["runtime_user"]["gid"])
    processes: list[ProcessIdentity] = []
    for peer in manifest["peers"]:
        require_path_unchanged(peer["working_directory"])
        require_path_unchanged(peer["storage"])
        pid_file = Path(peer["supervised_pid_file"])
        pid, pid_identity = parse_pid_file(pid_file)
        if pid_identity.uid != runtime_uid or pid_identity.gid != runtime_gid:
            raise MigrationError(f"supervised PID file owner changed: {pid_file}")
        process = process_inspector(pid)
        require_peer_command(process, binary, Path(peer["config"]["path"]))
        if process.uid != runtime_uid or process.gid != runtime_gid:
            raise MigrationError(
                f"supervised validator owner changed: pid {process.pid}"
            )
        if Path(process.cwd).resolve(strict=True) != Path(
            peer["working_directory"]["path"]
        ).resolve(strict=True):
            raise MigrationError(
                f"supervised validator working directory changed: pid {process.pid}"
            )
        processes.append(process)
    if len({process.pid for process in processes}) != PEER_COUNT:
        raise MigrationError("supervised PID files do not name four distinct peers")
    return processes


def wait_for_supervised_peers(
    manifest: dict[str, Any], timeout: int = 30
) -> list[ProcessIdentity]:
    """Tolerate an in-progress restart while confirming all new peer identities."""

    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            return inspect_supervised_peers_once(manifest)
        except (MigrationError, OSError) as exc:
            last_error = exc
            time.sleep(0.25)
    raise MigrationError(
        f"could not verify four supervised validator identities: {last_error}"
    )


def validate_apply_preflight(
    manifest: dict[str, Any],
    *,
    process_inspector: ProcessInspector = inspect_process,
) -> None:
    """Revalidate every mutable identity immediately before the cutover."""

    require_path_unchanged(manifest["base"])
    require_path_unchanged(manifest["genesis"])
    require_path_unchanged(manifest["binary"])
    require_path_unchanged(manifest["python"])
    require_path_unchanged(manifest["supervisor_source"])
    for runner in manifest["legacy"]["runners"]:
        require_path_unchanged(runner)
    require_process_unchanged(
        manifest["legacy"]["controller"], process_inspector=process_inspector
    )
    for peer in manifest["peers"]:
        require_path_unchanged(peer["legacy_pid_file"])
        require_path_unchanged(peer["config"])
        require_path_unchanged(peer["working_directory"])
        require_path_unchanged(peer["storage"])
        require_path_unchanged(peer["stores"]["kura"])
        require_path_unchanged(peer["stores"]["snapshot"])
        require_process_unchanged(
            peer["legacy_process"], process_inspector=process_inspector
        )


def apply_plan(args: argparse.Namespace) -> None:
    """Install and cut over to four independently supervised validators."""

    if args.confirm != CONFIRMATION:
        raise MigrationError(
            f"--confirm must be exactly {CONFIRMATION!r}; no changes were made"
        )
    if os.geteuid() != 0:
        raise MigrationError("apply requires root to install LaunchDaemons")
    manifest_path = normalize_absolute(args.manifest)
    manifest, stage, authenticated_assets = read_manifest(
        manifest_path, args.expected_manifest_sha256
    )
    validate_apply_preflight(manifest)

    runtime_user = manifest["runtime_user"]
    uid = int(runtime_user["uid"])
    gid = int(runtime_user["gid"])
    install_dir = Path(manifest["install"]["directory"])
    logs_dir = Path(manifest["install"]["logs_dir"])
    pids_dir = install_dir / "pids"
    terminal_dir = install_dir / "terminal"
    launch_daemons_dir = Path(manifest["install"]["launch_daemons_dir"])
    if str(launch_daemons_dir) != "/Library/LaunchDaemons":
        raise MigrationError(
            "apply only accepts /Library/LaunchDaemons; use plan output for rehearsal"
        )
    for peer in manifest["peers"]:
        destination = launch_daemons_dir / f"{peer['label']}.plist"
        if destination.exists() or destination.is_symlink():
            raise MigrationError(f"LaunchDaemon already exists: {destination}")
        result = subprocess.run(
            ["/bin/launchctl", "print", f"system/{peer['label']}"],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            raise MigrationError(f"launchd job is already loaded: {peer['label']}")
        run_checked(
            ["/usr/bin/plutil", "-lint", str(stage / "launchd" / destination.name)]
        )

    ensure_install_directory(install_dir, uid=uid, gid=gid, mode=0o700)
    ensure_install_directory(pids_dir, uid=uid, gid=gid, mode=0o700)
    ensure_install_directory(logs_dir, uid=uid, gid=gid, mode=0o700)
    ensure_install_directory(terminal_dir, uid=uid, gid=gid, mode=0o700)
    lifecycle_roots = ensure_lifecycle_journal_layout(
        install_dir, uid=uid, gid=gid
    )
    if lifecycle_roots != tuple(
        Path(peer["lifecycle"]["journal_root"]) for peer in manifest["peers"]
    ):
        raise MigrationError("installed lifecycle journal layout is not exact")
    installed_supervisor = Path(manifest["install"]["supervisor"])
    copy_new_file(
        authenticated_assets[SUPERVISOR_SOURCE.name],
        installed_supervisor,
        uid=uid,
        gid=gid,
        mode=0o700,
    )
    for peer in manifest["peers"]:
        source = stage / "launchd" / f"{peer['label']}.plist"
        destination = launch_daemons_dir / source.name
        relative = f"launchd/{source.name}"
        copy_new_file(
            authenticated_assets[relative],
            destination,
            uid=0,
            gid=0,
            mode=0o644,
        )

    controller = dict_to_sealed_process_identity(manifest["legacy"]["controller"])
    legacy_peers = [
        dict_to_sealed_process_identity(peer["legacy_process"])
        for peer in manifest["peers"]
    ]
    require_process_unchanged(manifest["legacy"]["controller"])
    for peer in manifest["peers"]:
        require_process_unchanged(peer["legacy_process"])
    # The legacy runner's EXIT trap intentionally terminates its four children.
    # This is the single scheduled all-peer cutover.  After it completes, each
    # validator has its own launchd job and can restart without affecting peers.
    os.kill(controller.pid, signal.SIGTERM)
    timeout = int(manifest["runtime"]["termination_timeout_seconds"])
    if not wait_processes_gone([controller, *legacy_peers], timeout):
        survivors = [
            process.pid
            for process in [controller, *legacy_peers]
            if process_is_alive(process.pid)
        ]
        raise MigrationError(
            "legacy topology did not stop cleanly; refusing SIGKILL and launchd "
            f"bootstrap, surviving pids={survivors}"
        )

    bootstrap_errors: list[str] = []
    for peer in manifest["peers"]:
        plist_path = launch_daemons_dir / f"{peer['label']}.plist"
        result = subprocess.run(
            ["/bin/launchctl", "bootstrap", "system", str(plist_path)],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip() or "no output"
            bootstrap_errors.append(f"{peer['label']}: {detail}")
    if bootstrap_errors:
        raise MigrationError(
            "one or more independent jobs failed to bootstrap; other jobs remain "
            "supervised: " + "; ".join(bootstrap_errors)
        )

    unhealthy = wait_healthy(
        manifest["peers"], int(manifest["runtime"]["health_timeout_seconds"])
    )
    if unhealthy:
        raise MigrationError(
            "independent jobs are loaded but Torii health timed out on ports "
            + ", ".join(str(port) for port in unhealthy)
        )
    supervised_processes = wait_for_supervised_peers(manifest)
    receipt = {
        "schema_version": SCHEMA_VERSION,
        "applied_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source_manifest_sha256": args.expected_manifest_sha256,
        "legacy_controller": manifest["legacy"]["controller"],
        "labels": [peer["label"] for peer in manifest["peers"]],
        "working_directories": [
            peer["working_directory"] for peer in manifest["peers"]
        ],
        "storage": [peer["storage"] for peer in manifest["peers"]],
        "supervised_processes": [
            process.sealed().as_dict() for process in supervised_processes
        ],
    }
    receipt_body = (
        json.dumps(receipt, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")
    receipt_path = install_dir / "migration-receipt.json"
    atomic_write(receipt_path, receipt_body, 0o600)
    os.chown(receipt_path, uid, gid)
    print(
        "Taira supervision migration applied; storage identities were preserved "
        f"and {len(manifest['peers'])} independent jobs are healthy."
    )


def add_plan_arguments(parser: argparse.ArgumentParser) -> None:
    """Register the read-only planning inputs."""

    parser.add_argument("--base", required=True, help="absolute deployed Taira base")
    parser.add_argument("--output-dir", required=True, help="new staging directory")
    parser.add_argument("--irohad", help="validator binary (default: BASE/bin/iroha3d)")
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="absolute Python executable recorded in LaunchDaemons",
    )
    parser.add_argument(
        "--config", action="append", help="repeat four times in peer order"
    )
    parser.add_argument(
        "--storage", action="append", help="repeat four times in peer order"
    )
    parser.add_argument(
        "--pid-file", action="append", help="repeat four times in peer order"
    )
    parser.add_argument(
        "--authenticated-node-binding",
        action="append",
        required=True,
        help=(
            "taira-validator-N=<deploy-authenticated receipt-signer node ID>; "
            "repeat exactly once for each validator slug"
        ),
    )
    parser.add_argument(
        "--legacy-runner",
        action="append",
        help="approved run-canonical.sh/launchd-run.sh path (repeatable)",
    )
    parser.add_argument("--install-dir", help="default: BASE/supervision")
    parser.add_argument("--logs-dir", help="default: BASE/supervision/logs")
    parser.add_argument(
        "--launch-daemons-dir",
        default="/Library/LaunchDaemons",
        help="render destination; apply requires the default system directory",
    )
    parser.add_argument("--label-prefix", default=DEFAULT_LABEL_PREFIX)
    parser.add_argument("--initial-backoff-seconds", type=float, default=1.0)
    parser.add_argument("--maximum-backoff-seconds", type=float, default=30.0)
    parser.add_argument("--stable-uptime-seconds", type=float, default=120.0)
    parser.add_argument("--rapid-fatal-uptime-seconds", type=float, default=30.0)
    parser.add_argument(
        "--restart-generation",
        help=(
            "optional lowercase SHA-256 generation token; changing it clears "
            "a prior identity-matched terminal-unhealthy latch"
        ),
    )
    parser.add_argument("--launchd-throttle-seconds", type=int, default=10)
    parser.add_argument("--termination-timeout-seconds", type=int, default=60)
    parser.add_argument("--health-timeout-seconds", type=int, default=900)
    parser.add_argument("--rust-log", default="info")
    parser.add_argument("--zk-halo2-enabled", choices=("true", "false"), default="true")


def build_parser() -> argparse.ArgumentParser:
    """Build the two-phase migration command line."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    plan = subparsers.add_parser(
        "plan", help="inspect legacy peers and write a read-only migration bundle"
    )
    add_plan_arguments(plan)
    apply_parser = subparsers.add_parser(
        "apply", help="revalidate and execute the scheduled launchd cutover"
    )
    apply_parser.add_argument("--manifest", required=True)
    apply_parser.add_argument("--expected-manifest-sha256", required=True)
    apply_parser.add_argument(
        "--confirm",
        required=True,
        help=f"must be exactly {CONFIRMATION}",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the requested planning or guarded apply phase."""

    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "plan":
            manifest, assets = create_plan(args)
            output_dir = normalize_absolute(args.output_dir)
            digest = write_plan(output_dir, manifest, assets)
            print(f"staged={output_dir}")
            print(f"manifest_sha256={digest}")
            print("No validator, controller, launchd job, or storage path was changed.")
        else:
            apply_plan(args)
    except (MigrationError, OSError, ValueError) as exc:
        print(f"Taira supervision migration refused: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
