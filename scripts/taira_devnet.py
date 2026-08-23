#!/usr/bin/env python3
"""Bring up, verify, inspect, or stop one disposable four-peer Taira devnet.

``up`` builds the current Kagami, fixed-FD Taira daemon, and CLI; asks Kagami
for a fresh four-validator NPoS Nexus network using the canonical Taira chain
id; validates all four configs; starts the peers; and proves finality with one
signed ``iroha tx ping`` submission followed by the typed transaction-status
waiter.
The opt-in full doctor remains read-only with respect to Inrou because
first-release shipping nodes reject Inrou hosting until mandatory confinement
is complete.
The generated network lives in one marked directory and is replaced on the
next ``up``.  There is no release authority, promotion state, evidence bundle,
soak, or rollback workflow.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any, NoReturn

try:
    from taira_constants import (
        CHAIN_ID as DEFAULT_CHAIN_ID,
        CHAIN_DISCRIMINANT as DEFAULT_CHAIN_DISCRIMINANT,
        PEER_COUNT,
        network_id_from_genesis_hash,
    )
except ModuleNotFoundError:
    from scripts.taira_constants import (
        CHAIN_ID as DEFAULT_CHAIN_ID,
        CHAIN_DISCRIMINANT as DEFAULT_CHAIN_DISCRIMINANT,
        PEER_COUNT,
        network_id_from_genesis_hash,
    )


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIR = REPO_ROOT / "dist" / "taira-devnet"
DEFAULT_API_PORT = 29_080
DEFAULT_P2P_PORT = 33_337
DEFAULT_OPERATION_TIMEOUT_SECONDS = 300
# Four optimized daemons plus the Nexus/AMX lane pipeline routinely need more
# than the ten-second view-zero deadline derived from Kagami's generic
# one-second localnet cadence.  The five-second proposal cadence deliberately
# trades a few seconds of smoke-test latency for a robust fifty-second
# view-zero deadline.
DEFAULT_BLOCK_CADENCE_MS = 5_000
MARKER = ".iroha-taira-devnet"
MARKER_BODY = "managed by scripts/taira_devnet.py\n"
MAX_BUNDLE_TEXT_BYTES = 8 * 1024 * 1024
MAX_LOG_TAIL_BYTES = 64 * 1024
MAX_HTTP_RESPONSE_BYTES = 1024 * 1024
MAX_MARKER_BYTES = 128
MAX_PID_FILE_BYTES = 32
BUILD_ENV_REMOVALS = ("CARGO_INCREMENTAL", "RUSTC_WRAPPER")
RUNTIME_SIGNER_DIRECTORY = Path("runtime") / "taira-runtime-signers"
RUNTIME_SIGNER_FILE_BYTES = 71
GENERATED_LOCALNET_NEXUS_STORAGE_BYTES = 1_073_741_824
TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES = 68_719_476_736
TAIRA_NEXUS_STORAGE_WEIGHTS = (
    ("kura_blocks_bps", 5_500),
    ("wsv_snapshots_bps", 2_000),
    ("sorafs_bps", 2_000),
    ("soranet_spool_bps", 250),
    ("soravpn_spool_bps", 250),
)
STORAGE_WEIGHT_BASIS_POINTS = 10_000
TAIRA_SORAFS_MAX_CAPACITY_BYTES = 13_743_895_347

# Keep this inventory beside the commands that consume the surfaces.  A
# successful `--help` is not sufficient: clap still accepts an existing parent
# command when one of the leaf options used below has drifted away.
CLI_SURFACES: tuple[tuple[str, tuple[str, ...], tuple[str, ...]], ...] = (
    (
        "kagami",
        ("localnet",),
        (
            "--out-dir",
            "--fresh-random-keys",
            "--sora-profile",
            "--consensus-mode",
            "--peers",
            "--bind-host",
            "--public-host",
            "--chain-id",
            "--base-api-port",
            "--base-p2p-port",
            "--block-cadence-ms",
        ),
    ),
    (
        "iroha3d_taira",
        (),
        ("--sora", "--config", "--genesis-manifest-json", "--check-config"),
    ),
    ("iroha", (), ("--config", "--machine", "--output-format", "--fee-payer")),
    ("iroha", ("tx", "ping"), ("--no-wait", "--log-level", "--msg")),
    (
        "iroha",
        ("tx", "status"),
        ("--hash", "--wait", "--timeout-ms", "--poll-interval-ms", "--terminal-status"),
    ),
)


class DevnetError(RuntimeError):
    """A disposable Taira operation failed."""


def fail(message: str) -> NoReturn:
    """Raise a concise operator-facing error."""

    raise DevnetError(message)


Runner = Callable[..., subprocess.CompletedProcess[str]]
Request = Callable[[str, object | None], tuple[int, object | None]]


def run_command(
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: float | None = None,
    capture_output: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run one command and surface its useful trailing diagnostics."""

    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            env=env,
            timeout=timeout,
            capture_output=capture_output,
            text=True,
            check=False,
        )
    except subprocess.TimeoutExpired:
        fail(f"{Path(command[0]).name} timed out after {timeout:g}s")
    if completed.returncode != 0:
        stderr = completed.stderr or ""
        stdout = completed.stdout or ""
        detail = (stderr.strip() or stdout.strip())[-6000:]
        fail(f"{Path(command[0]).name} failed: {detail or completed.returncode}")
    return completed


def submitted_transaction_hash(completed: subprocess.CompletedProcess[str]) -> str:
    """Extract the raw 32-byte hash accepted by ``iroha tx status``."""

    try:
        payload = json.loads(completed.stdout or "")
    except (TypeError, ValueError):
        fail("signed ping did not return its JSON transaction receipt")
    value = payload.get("hash") if isinstance(payload, dict) else None
    match = (
        re.fullmatch(r"hash:([0-9A-Fa-f]{64})#[0-9A-Fa-f]{4}", value)
        if isinstance(value, str)
        else None
    )
    if match is None:
        fail("signed ping returned an invalid transaction hash")
    return match.group(1)


def require_applied_transaction(
    completed: subprocess.CompletedProcess[str], expected_hash: str
) -> None:
    """Require the typed pipeline waiter to confirm the submitted transaction."""

    try:
        payload = json.loads(completed.stdout or "")
    except (TypeError, ValueError):
        fail("transaction status waiter did not return JSON")
    if not isinstance(payload, dict):
        fail("transaction status waiter returned an invalid response")
    actual_hash = payload.get("hash")
    terminal_kind = payload.get("terminal_kind")
    if (
        not isinstance(actual_hash, str)
        or actual_hash.lower() != expected_hash.lower()
        or terminal_kind != "Applied"
    ):
        fail("signed ping did not reach Applied pipeline finality")


def require_executable(path: Path) -> Path:
    """Require one regular executable file."""

    path = path.expanduser().absolute()
    if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK):
        fail(f"required executable is unavailable: {path}")
    return path


def managed_root(path: Path, *, create: bool) -> Path:
    """Resolve a narrowly marked directory owned by this script."""

    path = path.expanduser().absolute().resolve(strict=False)
    forbidden = {Path("/"), REPO_ROOT, Path.home().absolute()}
    if path in forbidden:
        fail(f"refusing unsafe devnet directory: {path}")
    marker = path / MARKER
    if not path.exists():
        if not create:
            fail(f"no Taira devnet exists at {path}; run `up` first")
        path.mkdir(parents=True, mode=0o700)
    if not path.is_dir():
        fail(f"devnet path is not a directory: {path}")
    if marker.exists():
        if marker.is_symlink() or read_bounded_text(
            marker,
            limit=MAX_MARKER_BYTES,
            label="devnet marker",
        ) != MARKER_BODY:
            fail(f"invalid devnet marker: {marker}")
    elif any(path.iterdir()):
        fail(f"refusing unmarked non-empty directory: {path}")
    elif create:
        marker.write_text(MARKER_BODY, encoding="utf-8")
        marker.chmod(0o600)
    else:
        fail(f"devnet marker is missing: {marker}")
    path.chmod(0o700)
    return path.resolve(strict=True)


def network_dir(root: Path) -> Path:
    """Return the sole disposable network directory below a managed root."""

    return root / "network"


def require_network_bundle(root: Path) -> Path:
    """Require the minimal generated files that identify this owned cohort."""

    target = network_dir(root)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    if not target.is_dir():
        fail(f"no generated Taira network exists at {target}; run `up` first")
    required = [
        target / "client.toml",
        target / "genesis.expected_hash",
        target / "start.sh",
        target / "stop.sh",
    ]
    required.extend(target / f"peer{index}.toml" for index in range(PEER_COUNT))
    for path in required:
        if path.is_symlink() or not path.is_file():
            fail(f"generated Taira network is incomplete: missing {path.name}")
    require_runtime_signer_files(target)
    return target


def runtime_signer_paths(target: Path) -> tuple[Path, ...]:
    """Return the four fixed runtime signer files without reading their contents."""

    return tuple(
        target / RUNTIME_SIGNER_DIRECTORY / f"peer{index}.private_key"
        for index in range(PEER_COUNT)
    )


def runtime_signer_launch_paths(target: Path) -> tuple[Path, ...]:
    """Return the four disposable FD198 launch copies without reading them."""

    return tuple(
        target / RUNTIME_SIGNER_DIRECTORY / f"peer{index}.fd198"
        for index in range(PEER_COUNT)
    )


def require_runtime_signer_files(target: Path) -> None:
    """Require four distinct owner-only single-link key files."""

    directory = target / RUNTIME_SIGNER_DIRECTORY
    if directory.is_symlink() or not directory.is_dir():
        fail(f"generated Taira runtime signer directory is missing: {directory}")
    identities: set[tuple[int, int]] = set()
    for path in runtime_signer_paths(target):
        if path.is_symlink():
            fail(f"refusing symlinked Taira runtime signer file: {path}")
        try:
            metadata = path.stat()
        except OSError as error:
            fail(f"cannot inspect Taira runtime signer file {path}: {error}")
        if (
            not path.is_file()
            or metadata.st_uid != os.geteuid()
            or metadata.st_mode & 0o7777 != 0o600
            or metadata.st_nlink != 1
            or metadata.st_size != RUNTIME_SIGNER_FILE_BYTES
        ):
            fail(f"untrusted Taira runtime signer file: {path}")
        identity = (metadata.st_dev, metadata.st_ino)
        if identity in identities:
            fail("Taira peers must not share a runtime signer file")
        identities.add(identity)


def delete_runtime_signer_files(target: Path) -> None:
    """Delete the stopped cohort's persistent keys and validated launch remnants."""

    require_runtime_signer_files(target)
    directory = target / RUNTIME_SIGNER_DIRECTORY
    source_paths = runtime_signer_paths(target)
    launch_paths = runtime_signer_launch_paths(target)
    expected = {path.name for path in (*source_paths, *launch_paths)}
    actual = {path.name for path in directory.iterdir()}
    if not actual.issubset(expected):
        fail(f"refusing unexpected Taira runtime signer directory contents: {directory}")
    for path in launch_paths:
        if path.is_symlink():
            fail(f"refusing symlinked Taira FD198 launch file: {path}")
        if not path.exists():
            continue
        try:
            metadata = path.stat()
        except OSError as error:
            fail(f"cannot inspect Taira FD198 launch file {path}: {error}")
        if (
            not path.is_file()
            or metadata.st_uid != os.geteuid()
            or metadata.st_mode & 0o7777 != 0o600
            or metadata.st_nlink != 1
            or metadata.st_size not in (0, RUNTIME_SIGNER_FILE_BYTES)
        ):
            fail(f"untrusted Taira FD198 launch file: {path}")
        path.unlink()
    for path in source_paths:
        path.unlink()
    directory.rmdir()


def require_stoppable_network(root: Path) -> Path:
    """Require the generated stop surface without depending on intact configs."""

    target = network_dir(root)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    if not target.is_dir():
        fail(f"no generated Taira network exists at {target}; run `up` first")
    stop = target / "stop.sh"
    if stop.is_symlink() or not stop.is_file():
        fail(f"generated Taira network is incomplete: missing {stop.name}")
    return target


def read_bounded_text(path: Path, *, limit: int, label: str) -> str:
    """Read one regular bundle file without accepting an oversized substitute."""

    if path.is_symlink() or not path.is_file():
        fail(f"{label} is missing or not a regular file: {path}")
    try:
        size = path.stat().st_size
    except OSError as error:
        fail(f"cannot inspect {label} {path}: {error}")
    if size > limit:
        fail(f"{label} exceeds the {limit}-byte safety bound: {path}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        fail(f"cannot read {label} {path}: {error}")


def quoted_assignment(path: Path, key: str) -> str:
    """Read one unique canonical quoted assignment from a generated TOML file."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    pattern = re.compile(rf'^\s*{re.escape(key)}\s*=\s*"([^"\\]*)"\s*$')
    values = [match.group(1) for line in text.splitlines() if (match := pattern.fullmatch(line))]
    if len(values) != 1:
        fail(f"generated config must contain one canonical {key} assignment: {path}")
    return values[0]


def integer_assignment(path: Path, key: str) -> int:
    """Read one unique canonical non-negative integer assignment from TOML."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    pattern = re.compile(rf"^\s*{re.escape(key)}\s*=\s*(0|[1-9][0-9]*)\s*$")
    values = [
        int(match.group(1))
        for line in text.splitlines()
        if (match := pattern.fullmatch(line))
    ]
    if len(values) != 1:
        fail(f"generated config must contain one canonical {key} assignment: {path}")
    return values[0]


def require_bundle_identity(target: Path, roots: Sequence[str]) -> None:
    """Bind checks to the generated Taira chain and requested loopback ports."""

    client = target / "client.toml"
    if quoted_assignment(client, "chain") != DEFAULT_CHAIN_ID:
        fail(f"generated client config is not for canonical Taira: {client}")
    if (
        integer_assignment(client, "chain_discriminant")
        != DEFAULT_CHAIN_DISCRIMINANT
    ):
        fail(f"generated client config has the wrong Taira chain discriminant: {client}")
    if quoted_assignment(client, "torii_url") != roots[0]:
        fail(f"generated client Torii URL does not match requested ports: {client}")
    expected_hash = read_bounded_text(
        target / "genesis.expected_hash",
        limit=256,
        label="generated genesis hash",
    ).strip()
    try:
        expected_network_id = network_id_from_genesis_hash(expected_hash)
    except ValueError as error:
        fail(f"generated genesis hash is invalid: {target / 'genesis.expected_hash'}: {error}")
    if quoted_assignment(client, "network_id") != expected_network_id:
        fail(f"generated client network id does not match its genesis hash: {client}")

    for index, root in enumerate(roots):
        config = target / f"peer{index}.toml"
        if quoted_assignment(config, "chain") != DEFAULT_CHAIN_ID:
            fail(f"peer{index} config is not for canonical Taira: {config}")
        if (
            integer_assignment(config, "chain_discriminant")
            != DEFAULT_CHAIN_DISCRIMINANT
        ):
            fail(f"peer{index} config has the wrong Taira chain discriminant: {config}")
        if quoted_assignment(config, "expected_hash") != expected_network_id:
            fail(f"peer{index} config genesis hash does not match the generated bundle: {config}")
        port = root.removeprefix("http://127.0.0.1:").removesuffix("/")
        address = re.compile(
            rf'^address = "addr:127\.0\.0\.1:{re.escape(port)}#[0-9A-Fa-f]{{4}}"$'
        )
        text = read_bounded_text(
            config,
            limit=MAX_BUNDLE_TEXT_BYTES,
            label=f"peer{index} config",
        )
        if sum(address.fullmatch(line) is not None for line in text.splitlines()) != 1:
            fail(f"peer{index} Torii address does not match requested ports: {config}")


def process_table(run: Runner) -> dict[int, str]:
    """Read the local process table used to bind PID files to peer configs."""

    completed = run(["ps", "-axww", "-o", "pid=,command="], timeout=5)
    processes: dict[int, str] = {}
    for line in (completed.stdout or "").splitlines():
        fields = line.strip().split(maxsplit=1)
        if len(fields) != 2 or not fields[0].isdigit():
            continue
        processes[int(fields[0])] = fields[1]
    return processes


def read_peer_pid(path: Path) -> int:
    """Read one small, regular, positive peer PID file."""

    value = read_bounded_text(path, limit=MAX_PID_FILE_BYTES, label="peer PID").strip()
    if not value.isdigit() or int(value) <= 1:
        fail(f"peer PID file is malformed: {path}")
    return int(value)


def command_uses_config(command: str, config: Path) -> bool:
    """Return whether one daemon argv owns exactly one exact peer config."""

    try:
        argv = shlex.split(command)
    except ValueError:
        return False
    if not argv or Path(argv[0]).name != "iroha3d_taira":
        return False
    configs: list[str] = []
    for index, argument in enumerate(argv):
        if argument == "--config":
            if index + 1 >= len(argv):
                return False
            configs.append(argv[index + 1])
        elif argument.startswith("--config="):
            configs.append(argument.removeprefix("--config="))
    return configs == [str(config)]


def require_running_cohort(target: Path, run: Runner) -> None:
    """Require exactly the four PID-bound processes generated for this bundle."""

    pids: list[int] = []
    for index in range(PEER_COUNT):
        config = target / f"peer{index}.toml"
        if config.is_symlink() or not config.is_file():
            fail(f"generated peer config is missing or unsafe: {config}")
        pids.append(read_peer_pid(target / f"peer{index}.pid"))
    if len(set(pids)) != PEER_COUNT:
        fail("generated peer PID files do not identify four distinct processes")

    processes = process_table(run)
    for index, pid in enumerate(pids):
        config = target / f"peer{index}.toml"
        matches = [
            process_pid
            for process_pid, command in processes.items()
            if command_uses_config(command, config)
        ]
        if matches != [pid]:
            fail(
                f"peer{index} PID {pid} is not the sole running process for its generated config"
            )


def require_stopped_cohort(target: Path, run: Runner) -> None:
    """Prove that teardown left neither peer PID files nor managed processes."""

    residual_pidfiles = sorted(path.name for path in target.glob("peer*.pid"))
    if residual_pidfiles:
        fail(f"Taira teardown left peer PID files: {', '.join(residual_pidfiles)}")
    processes = process_table(run)
    residual = [
        pid
        for pid, command in processes.items()
        if any(
            command_uses_config(command, target / f"peer{index}.toml")
            for index in range(PEER_COUNT)
        )
    ]
    if residual:
        fail(f"Taira teardown left managed peer processes running: {residual}")


def stop_network(root: Path, run: Runner, *, tolerate_failure: bool = False) -> None:
    """Stop only peers owned by the generated Kagami bundle."""

    try:
        target = network_dir(root)
        if target.is_symlink():
            fail(f"refusing symlinked network directory: {target}")
        if not target.exists():
            return
        if not target.is_dir():
            fail(f"network path is not a directory: {target}")
        pid_paths = [target / f"peer{index}.pid" for index in range(PEER_COUNT)]
        present_pid_paths = [
            path for path in pid_paths if path.exists() or path.is_symlink()
        ]
        if not present_pid_paths:
            require_stopped_cohort(target, run)
            return
        if len(present_pid_paths) != PEER_COUNT:
            fail(
                "Taira teardown left peer PID files: "
                + ", ".join(path.name for path in present_pid_paths)
            )
        # The generated stop script has process-control authority. Do not run
        # it until all four PID files, daemon argvs, and exact config paths
        # prove that the live cohort is ours.
        require_running_cohort(target, run)
        stop = target / "stop.sh"
        if stop.is_symlink() or not stop.is_file():
            fail(f"generated Taira network is incomplete: missing safe {stop.name}")
        run(["/bin/bash", str(stop)], cwd=stop.parent, timeout=30)
        require_stopped_cohort(target, run)
    except (DevnetError, subprocess.TimeoutExpired) as error:
        if not tolerate_failure:
            raise
        print(f"warning: could not prove Taira cohort stopped: {error}", file=sys.stderr)


def reset_network(root: Path, run: Runner) -> Path:
    """Stop and replace the one script-owned throwaway network directory."""

    target = network_dir(root)
    stop_network(root, run, tolerate_failure=False)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    if target.exists():
        if not target.is_dir():
            fail(f"network path is not a directory: {target}")
        shutil.rmtree(target)
    return target


def cargo_build_command(profile: str, target_dir: Path) -> list[str]:
    """Return the current-workspace build needed by the shipping smoke."""

    return [
        str(REPO_ROOT / "scripts" / "cargo_fast.sh"),
        "--target-dir",
        str(target_dir),
        "--stable-local-metadata",
        "--no-sccache",
        "--",
        "build",
        "--locked",
        "--profile",
        profile,
        "-p",
        "iroha_kagami",
        "--bin",
        "kagami",
        "-p",
        "irohad",
        "--bin",
        "iroha3d_taira",
        "-p",
        "iroha_cli",
        "--bin",
        "iroha",
    ]


def cargo_build_env() -> dict[str, str]:
    """Return an environment consistent with ``cargo_fast --no-sccache``."""

    env = os.environ.copy()
    for name in BUILD_ENV_REMOVALS:
        env.pop(name, None)
    return env


def binary_paths(args: argparse.Namespace, run: Runner) -> tuple[Path, Path, Path]:
    """Build or locate the exact-revision binaries needed by this invocation."""

    if args.bin_dir is not None and not args.no_build:
        fail("--bin-dir requires --no-build so a current build cannot be silently ignored")
    target_dir = args.target_dir.expanduser().absolute()
    if not args.no_build:
        print(f"Building current Taira binaries ({args.profile})...", flush=True)
        run(
            cargo_build_command(args.profile, target_dir),
            cwd=REPO_ROOT,
            env=cargo_build_env(),
            timeout=args.build_timeout_seconds,
            capture_output=False,
        )
    bin_dir = (
        args.bin_dir.expanduser().absolute()
        if args.bin_dir is not None
        else target_dir / args.profile
    )
    return tuple(
        require_executable(bin_dir / name)
        for name in ("kagami", "iroha3d_taira", "iroha")
    )


def help_has_option(help_text: str, option: str) -> bool:
    """Match one complete long option without accepting a longer lookalike."""

    return re.search(rf"(?<![\w-]){re.escape(option)}(?![\w-])", help_text) is not None


def preflight_cli_surfaces(
    kagami: Path,
    irohad: Path,
    iroha: Path,
    run: Runner,
    *,
    full_doctor: bool,
) -> None:
    """Prove every compiled command used by ``up`` before replacing a cohort."""

    binaries: dict[str, Path] = {
        "kagami": kagami,
        "iroha3d_taira": irohad,
        "iroha": iroha,
    }
    surfaces = list(CLI_SURFACES)
    if full_doctor:
        surfaces.append(
            ("iroha", ("taira", "doctor"), ("--public-root", "--json"))
        )
    for binary_name, subcommands, required_options in surfaces:
        command = [str(binaries[binary_name]), *subcommands, "--help"]
        completed = run(command, cwd=REPO_ROOT, timeout=20)
        help_text = "\n".join((completed.stdout or "", completed.stderr or ""))
        missing = [
            option for option in required_options if not help_has_option(help_text, option)
        ]
        if missing:
            surface = " ".join((binary_name, *subcommands))
            fail(
                f"compiled CLI surface `{surface}` is missing current options: "
                + ", ".join(missing)
            )


def generate_network(
    target: Path,
    kagami: Path,
    api_port: int,
    p2p_port: int,
    block_cadence_ms: int,
    run: Runner,
) -> None:
    """Generate exactly one fresh-key, four-validator Taira network."""

    run(
        [
            str(kagami),
            "localnet",
            "--out-dir",
            str(target),
            "--fresh-random-keys",
            "--sora-profile",
            "nexus",
            "--consensus-mode",
            "npos",
            "--peers",
            str(PEER_COUNT),
            "--bind-host",
            "127.0.0.1",
            "--public-host",
            "127.0.0.1",
            "--chain-id",
            DEFAULT_CHAIN_ID,
            "--base-api-port",
            str(api_port),
            "--base-p2p-port",
            str(p2p_port),
            "--block-cadence-ms",
            str(block_cadence_ms),
        ],
        cwd=REPO_ROOT,
        timeout=None,
        capture_output=False,
    )


def validate_configs(target: Path, irohad: Path, run: Runner) -> None:
    """Run the current daemon's offline validator for every generated peer."""

    require_canonical_taira_storage_profiles(target)
    for index in range(PEER_COUNT):
        config = target / f"peer{index}.toml"
        run(
            [
                str(irohad),
                "--sora",
                "--config",
                str(config),
                "--genesis-manifest-json",
                str(target / "genesis.json"),
                "--check-config",
            ],
            cwd=target,
            timeout=120,
        )


def http_request(url: str, payload: object | None = None) -> tuple[int, object | None]:
    """Send one local Torii GET/JSON POST and decode JSON when present."""

    body = None if payload is None else json.dumps(payload).encode("utf-8")
    plain_text_probe = url.rstrip("/").endswith(("/health", "/readyz"))
    headers = {"Accept": "text/plain" if plain_text_probe else "application/json"}
    if body is not None:
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(url, data=body, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=3) as response:
            status = response.status
            body = response.read(MAX_HTTP_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as error:
        return error.code, None
    except (OSError, ValueError):
        return 0, None
    if len(body) > MAX_HTTP_RESPONSE_BYTES:
        fail(f"HTTP response exceeds the {MAX_HTTP_RESPONSE_BYTES}-byte safety bound: {url}")
    if not body:
        return status, None
    try:
        return status, json.loads(body)
    except (UnicodeDecodeError, ValueError):
        return status, body.decode("utf-8", errors="replace")


def torii_roots(api_port: int) -> tuple[str, ...]:
    """Return all four loopback Torii roots."""

    return tuple(f"http://127.0.0.1:{api_port + index}/" for index in range(PEER_COUNT))


def bundle_torii_roots(target: Path) -> tuple[str, ...]:
    """Derive the owned cohort's Torii roots from its generated client config."""

    client = target / "client.toml"
    value = quoted_assignment(client, "torii_url")
    match = re.fullmatch(r"http://127\.0\.0\.1:([0-9]{1,5})/", value)
    if match is None:
        fail(f"generated client Torii URL is not a canonical loopback root: {client}")
    base_port = int(match.group(1))
    if base_port == 0 or base_port + PEER_COUNT - 1 > 65_535:
        fail(f"generated client Torii port cannot address four peers: {client}")
    return torii_roots(base_port)


def read_height(root: str, request: Request) -> int:
    """Read the canonical committed block height from ``/status/blocks``."""

    status, payload = request(root + "status/blocks", None)
    if status != 200 or isinstance(payload, bool) or not isinstance(payload, int):
        fail(f"invalid /status/blocks response from {root} (HTTP {status})")
    return payload


def check_sumeragi_status(root: str, request: Request) -> None:
    """Fail on authoritative restart or watchdog blockers when JSON is exposed."""

    url = root + "v1/sumeragi/status"
    status, payload = request(url, None)
    # Operator status can be protected or unavailable during early startup.
    # Health, readiness, height convergence, and the signed smoke remain the
    # mandatory portable surface; inspect the richer status whenever Torii
    # actually exposes its current JSON representation.
    if status != 200:
        return
    if not isinstance(payload, dict):
        fail(f"invalid Sumeragi status response from {root} (HTTP {status})")
    restart_required = payload.get("restart_required")
    if not isinstance(restart_required, bool):
        fail(f"Sumeragi status omitted boolean restart_required at {root}")
    if restart_required:
        fail(f"Sumeragi consensus requires restart at {root}")
    liveness = payload.get("liveness")
    if not isinstance(liveness, dict):
        fail(f"Sumeragi status omitted liveness diagnostics at {root}")
    blocker = liveness.get("blocker")
    if blocker is None:
        return
    blocker_name = blocker.get("blocker") if isinstance(blocker, dict) else None
    if not isinstance(blocker_name, str) or not blocker_name:
        fail(f"Sumeragi status returned an invalid liveness blocker at {root}")
    fail(f"Sumeragi liveness blocker at {root}: {blocker_name}")


def wait_for_cluster(
    roots: Sequence[str],
    timeout: float,
    request: Request,
    *,
    above: int | None = None,
) -> list[int]:
    """Wait for four ready peers at one converged height, optionally advanced."""

    deadline = time.monotonic() + timeout
    last = "not reachable"
    while time.monotonic() < deadline:
        # These probes ignore an unavailable/protected status route but make a
        # published fail-stop or watchdog blocker terminal immediately.  Keep
        # them outside the retryable readiness block so a serious consensus
        # diagnosis is not hidden behind a generic convergence timeout.
        for root in roots:
            check_sumeragi_status(root, request)
        try:
            for root in roots:
                for endpoint in ("health", "readyz"):
                    status, _ = request(root + endpoint, None)
                    if not 200 <= status < 300:
                        fail(f"{root}{endpoint} returned HTTP {status}")
            heights = [read_height(root, request) for root in roots]
            if len(set(heights)) == 1 and (above is None or heights[0] > above):
                return heights
            last = f"heights={heights}, required_above={above}"
        except DevnetError as error:
            last = str(error)
        time.sleep(0.5)
    fail(f"four-peer cluster did not converge within {timeout:g}s: {last}")


def check_mcp(root: str, request: Request) -> None:
    """Verify the enabled MCP endpoint can initialize and list current tools."""

    url = root + "v1/mcp"
    status, capabilities = request(url, None)
    protocol_version = (
        capabilities.get("protocolVersion") if isinstance(capabilities, dict) else None
    )
    if (
        status != 200
        or not isinstance(capabilities, dict)
        or capabilities.get("enabled") is not True
        or not isinstance(protocol_version, str)
        or not protocol_version.strip()
    ):
        fail(f"MCP capabilities are not enabled/current at {url} (HTTP {status})")
    initialize = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": {"name": "taira-devnet-smoke", "version": "1"},
        },
    }
    status, initialized = request(url, initialize)
    initialized_result = (
        initialized.get("result") if isinstance(initialized, dict) else None
    )
    if (
        status != 200
        or not isinstance(initialized, dict)
        or initialized.get("jsonrpc") != "2.0"
        or initialized.get("id") != 1
        or "error" in initialized
        or not isinstance(initialized_result, dict)
        or initialized_result.get("protocolVersion") != protocol_version
    ):
        fail(f"MCP initialize failed at {url} (HTTP {status})")
    initialized_notification = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }
    status, notification_response = request(url, initialized_notification)
    if status != 202 or notification_response is not None:
        fail(f"MCP initialized notification failed at {url} (HTTP {status})")
    tools_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {},
    }
    status, tools_response = request(url, tools_request)
    result = tools_response.get("result") if isinstance(tools_response, dict) else None
    tools = result.get("tools") if isinstance(result, dict) else None
    if (
        status != 200
        or not isinstance(tools_response, dict)
        or tools_response.get("jsonrpc") != "2.0"
        or tools_response.get("id") != 2
        or "error" in tools_response
        or not isinstance(tools, list)
        or not tools
        or any(
            not isinstance(tool, dict)
            or not isinstance(tool.get("name"), str)
            or not tool["name"].startswith("iroha.")
            for tool in tools
        )
    ):
        fail(f"MCP tools/list returned no tools at {url} (HTTP {status})")


def check_all_mcp(roots: Sequence[str], request: Request) -> None:
    """Verify the live MCP handshake and curated tools on every validator."""

    for root in roots:
        check_mcp(root, request)


def run_full_doctor(target: Path, iroha: Path, root: str, run: Runner) -> None:
    """Run the broad public-product diagnostic only when explicitly requested."""

    run(
        [
            str(iroha),
            "-c",
            str(target / "client.toml"),
            "taira",
            "doctor",
            "--public-root",
            root,
            "--json",
        ],
        cwd=target,
        timeout=120,
    )


def section_assignment(path: Path, section: str, key: str) -> str:
    """Read one unescaped scalar assignment from one exact generated TOML section."""

    text = read_bounded_text(path, limit=MAX_BUNDLE_TEXT_BYTES, label="generated config")
    header = re.compile(r"^\s*\[([^]]+)]\s*$")
    quoted = re.compile(rf'^\s*{re.escape(key)}\s*=\s*"([^"\\]*)"\s*$')
    bare = re.compile(rf"^\s*{re.escape(key)}\s*=\s*([^#\s]+)\s*$")
    current: str | None = None
    values: list[str] = []
    for line in text.splitlines():
        if match := header.fullmatch(line):
            current = match.group(1)
            continue
        if current != section:
            continue
        if match := quoted.fullmatch(line):
            values.append(match.group(1))
        elif match := bare.fullmatch(line):
            values.append(match.group(1))
    if len(values) != 1:
        fail(f"generated config must contain one {section}.{key} assignment: {path}")
    return values[0]


_CANONICAL_TOML_HEADER = re.compile(
    r"^\s*(\[\[|\[)([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)(\]\]|\])\s*$"
)
_CANONICAL_TOML_ASSIGNMENT = re.compile(
    r"^\s*([A-Za-z][A-Za-z0-9_-]*)\s*=\s*(\S(?:.*\S)?)\s*$"
)
_NEXUS_STORAGE_SECTION = "nexus.storage"
_NEXUS_STORAGE_WEIGHTS_SECTION = "nexus.storage.disk_budget_weights"
_SORAFS_STORAGE_SECTION = "sorafs.storage"


def _generated_config_sections(
    path: Path, text: str
) -> tuple[list[str], list[tuple[str, bool, int, int]]]:
    """Split canonical Kagami TOML into bounded table sections."""

    lines = text.splitlines(keepends=True)
    headers: list[tuple[str, bool, int]] = []
    for index, line in enumerate(lines):
        if not line.lstrip().startswith("["):
            continue
        match = _CANONICAL_TOML_HEADER.fullmatch(line.rstrip("\r\n"))
        if match is None or (match.group(1) == "[[") != (match.group(3) == "]]"):
            fail(f"generated config contains an unexpected TOML section header: {path}")
        headers.append((match.group(2), match.group(1) == "[[", index))
    sections = [
        (
            name,
            is_array,
            start,
            headers[offset + 1][2] if offset + 1 < len(headers) else len(lines),
        )
        for offset, (name, is_array, start) in enumerate(headers)
    ]
    return lines, sections


def _storage_section_assignments(
    path: Path,
    lines: Sequence[str],
    section: tuple[str, bool, int, int],
) -> dict[str, str]:
    """Read the exact scalar assignments from one generated storage table."""

    name, _, start, end = section
    assignments: dict[str, str] = {}
    for line in lines[start + 1 : end]:
        if not line.strip():
            continue
        match = _CANONICAL_TOML_ASSIGNMENT.fullmatch(line.rstrip("\r\n"))
        if match is None:
            fail(f"generated {name} contains an unexpected entry: {path}")
        key, value = match.groups()
        if key in assignments:
            fail(f"generated {name} contains duplicate `{key}` assignments: {path}")
        assignments[key] = value
    return assignments


def _one_storage_section(
    path: Path,
    sections: Sequence[tuple[str, bool, int, int]],
    name: str,
) -> tuple[str, bool, int, int]:
    """Require one non-array generated storage table with the exact name."""

    matches = [section for section in sections if section[0] == name]
    if len(matches) != 1 or matches[0][1]:
        fail(f"generated config must contain one [{name}] table: {path}")
    return matches[0]


def _require_exact_keys(
    path: Path,
    section: str,
    assignments: dict[str, str],
    expected: set[str],
) -> None:
    """Reject missing or additional assignments in a generated storage table."""

    actual = set(assignments)
    if actual != expected:
        missing = ", ".join(sorted(expected - actual)) or "none"
        unexpected = ", ".join(sorted(actual - expected)) or "none"
        fail(
            f"generated [{section}] has the wrong assignment set "
            f"(missing: {missing}; unexpected: {unexpected}): {path}"
        )


def _canonical_nonnegative_integer(path: Path, field: str, value: str) -> int:
    """Decode one canonical decimal integer from the generated overlay."""

    if re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        fail(f"generated {field} must be one canonical non-negative integer: {path}")
    return int(value)


def _expected_peer_sorafs_dir(target: Path, peer_index: int) -> Path:
    """Return one peer's disjoint canonical SoraFS store root."""

    return (target / "state" / f"peer{peer_index}" / "sorafs").resolve(
        strict=False
    )


def _storage_sections_for_mode(
    path: Path,
    text: str,
    *,
    canonical: bool,
) -> tuple[
    list[str],
    dict[str, tuple[str, bool, int, int]],
    dict[str, dict[str, str]],
]:
    """Require only the exact source or overlaid storage table topology."""

    lines, sections = _generated_config_sections(path, text)
    allowed = {
        _NEXUS_STORAGE_SECTION,
        _SORAFS_STORAGE_SECTION,
    }
    if canonical:
        allowed.add(_NEXUS_STORAGE_WEIGHTS_SECTION)
    related = [
        section
        for section in sections
        if section[0] == _NEXUS_STORAGE_SECTION
        or section[0].startswith(f"{_NEXUS_STORAGE_SECTION}.")
        or section[0] == _SORAFS_STORAGE_SECTION
        or section[0].startswith(f"{_SORAFS_STORAGE_SECTION}.")
    ]
    unexpected = sorted({section[0] for section in related if section[0] not in allowed})
    if unexpected:
        fail(
            "generated config contains unexpected storage sections "
            f"{', '.join(f'[{name}]' for name in unexpected)}: {path}"
        )
    selected = {
        name: _one_storage_section(path, sections, name)
        for name in sorted(allowed)
    }
    assignments = {
        name: _storage_section_assignments(path, lines, section)
        for name, section in selected.items()
    }
    return lines, selected, assignments


def _validate_generated_storage_source(
    config: Path,
    target: Path,
    peer_index: int,
) -> tuple[list[str], dict[str, tuple[str, bool, int, int]]]:
    """Require the exact current Kagami storage shape before replacing it."""

    text = read_bounded_text(
        config,
        limit=MAX_BUNDLE_TEXT_BYTES,
        label=f"peer{peer_index} config",
    )
    lines, sections, assignments = _storage_sections_for_mode(
        config, text, canonical=False
    )
    nexus = assignments[_NEXUS_STORAGE_SECTION]
    _require_exact_keys(
        config,
        _NEXUS_STORAGE_SECTION,
        nexus,
        {"local_budget_bytes"},
    )
    if (
        _canonical_nonnegative_integer(
            config,
            "nexus.storage.local_budget_bytes",
            nexus["local_budget_bytes"],
        )
        != GENERATED_LOCALNET_NEXUS_STORAGE_BYTES
    ):
        fail(f"generated [{_NEXUS_STORAGE_SECTION}] is not the expected localnet shape: {config}")

    sorafs = assignments[_SORAFS_STORAGE_SECTION]
    _require_exact_keys(
        config,
        _SORAFS_STORAGE_SECTION,
        sorafs,
        {"data_dir", "enabled"},
    )
    expected_dir = _expected_peer_sorafs_dir(target, peer_index)
    if sorafs["enabled"] != "false" or sorafs["data_dir"] != f'"{expected_dir}"':
        fail(f"generated [{_SORAFS_STORAGE_SECTION}] is not the expected localnet shape: {config}")
    return lines, sections


def _canonical_storage_text(
    config: Path,
    target: Path,
    peer_index: int,
) -> str:
    """Render one fail-closed canonical Taira V1 storage overlay."""

    lines, sections = _validate_generated_storage_source(config, target, peer_index)
    nexus = sections[_NEXUS_STORAGE_SECTION]
    sorafs = sections[_SORAFS_STORAGE_SECTION]
    data_dir = _expected_peer_sorafs_dir(target, peer_index)
    nexus_text = (
        f"[{_NEXUS_STORAGE_SECTION}]\n"
        f"local_budget_bytes = {TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES}\n\n"
        f"[{_NEXUS_STORAGE_WEIGHTS_SECTION}]\n"
        + "".join(f"{key} = {value}\n" for key, value in TAIRA_NEXUS_STORAGE_WEIGHTS)
        + "\n"
    )
    sorafs_text = (
        f"[{_SORAFS_STORAGE_SECTION}]\n"
        f'data_dir = "{data_dir}"\n'
        "enabled = false\n"
        f"max_capacity_bytes = {TAIRA_SORAFS_MAX_CAPACITY_BYTES}\n\n"
    )
    replacements = {
        nexus[2]: (nexus[3], nexus_text),
        sorafs[2]: (sorafs[3], sorafs_text),
    }
    rendered: list[str] = []
    cursor = 0
    for start in sorted(replacements):
        end, replacement = replacements[start]
        rendered.extend(lines[cursor:start])
        rendered.append(replacement)
        cursor = end
    rendered.extend(lines[cursor:])
    return "".join(rendered)


def _atomic_replace_generated_config(path: Path, text: str) -> None:
    """Replace one generated config without exposing a partially written file."""

    metadata = path.stat()
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.storage-overlay-",
    )
    temporary = Path(temporary_name)
    try:
        os.fchmod(descriptor, metadata.st_mode & 0o7777)
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as output:
            output.write(text)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def require_canonical_taira_storage_profiles(target: Path) -> None:
    """Validate the exact four-peer Taira V1 storage profile and cap math."""

    expected_files = {f"peer{index}.toml" for index in range(PEER_COUNT)}
    actual_files = {path.name for path in target.glob("peer*.toml")}
    if actual_files != expected_files:
        fail("generated Taira network must contain exactly peer0.toml through peer3.toml")
    for peer_index in range(PEER_COUNT):
        config = target / f"peer{peer_index}.toml"
        text = read_bounded_text(
            config,
            limit=MAX_BUNDLE_TEXT_BYTES,
            label=f"peer{peer_index} config",
        )
        _, _, assignments = _storage_sections_for_mode(config, text, canonical=True)
        nexus = assignments[_NEXUS_STORAGE_SECTION]
        weights = assignments[_NEXUS_STORAGE_WEIGHTS_SECTION]
        sorafs = assignments[_SORAFS_STORAGE_SECTION]
        _require_exact_keys(
            config,
            _NEXUS_STORAGE_SECTION,
            nexus,
            {"local_budget_bytes"},
        )
        expected_weight_fields = {key for key, _ in TAIRA_NEXUS_STORAGE_WEIGHTS}
        _require_exact_keys(
            config,
            _NEXUS_STORAGE_WEIGHTS_SECTION,
            weights,
            expected_weight_fields,
        )
        _require_exact_keys(
            config,
            _SORAFS_STORAGE_SECTION,
            sorafs,
            {"data_dir", "enabled", "max_capacity_bytes"},
        )
        aggregate = _canonical_nonnegative_integer(
            config,
            "nexus.storage.local_budget_bytes",
            nexus["local_budget_bytes"],
        )
        parsed_weights = {
            key: _canonical_nonnegative_integer(
                config,
                f"nexus.storage.disk_budget_weights.{key}",
                weights[key],
            )
            for key in expected_weight_fields
        }
        capacity = _canonical_nonnegative_integer(
            config,
            "sorafs.storage.max_capacity_bytes",
            sorafs["max_capacity_bytes"],
        )
        expected_dir = _expected_peer_sorafs_dir(target, peer_index)
        if aggregate != TAIRA_NEXUS_STORAGE_AGGREGATE_BYTES:
            fail(f"peer{peer_index} has the wrong Taira storage aggregate: {config}")
        parsed_weight_tuple = tuple(
            (key, parsed_weights[key]) for key, _ in TAIRA_NEXUS_STORAGE_WEIGHTS
        )
        if parsed_weight_tuple != TAIRA_NEXUS_STORAGE_WEIGHTS:
            fail(f"peer{peer_index} has the wrong Taira storage weights: {config}")
        if sum(parsed_weights.values()) != STORAGE_WEIGHT_BASIS_POINTS:
            fail(f"peer{peer_index} Taira storage weights do not sum to 10000 bps: {config}")
        computed_capacity = (
            aggregate * parsed_weights["sorafs_bps"] // STORAGE_WEIGHT_BASIS_POINTS
        )
        if computed_capacity != TAIRA_SORAFS_MAX_CAPACITY_BYTES or capacity != computed_capacity:
            fail(f"peer{peer_index} has the wrong computed SoraFS capacity: {config}")
        if sorafs["enabled"] != "false" or sorafs["data_dir"] != f'"{expected_dir}"':
            fail(f"peer{peer_index} does not use its disabled disjoint SoraFS root: {config}")


def apply_canonical_taira_storage_profiles(target: Path) -> None:
    """Atomically overlay all four generated configs, then validate the result."""

    expected_files = {f"peer{index}.toml" for index in range(PEER_COUNT)}
    actual_files = {path.name for path in target.glob("peer*.toml")}
    if actual_files != expected_files:
        fail("generated Taira network must contain exactly peer0.toml through peer3.toml")
    replacements = [
        (
            target / f"peer{peer_index}.toml",
            _canonical_storage_text(
                target / f"peer{peer_index}.toml",
                target,
                peer_index,
            ),
        )
        for peer_index in range(PEER_COUNT)
    ]
    for config, text in replacements:
        _atomic_replace_generated_config(config, text)
    require_canonical_taira_storage_profiles(target)


def dump_logs(target: Path) -> None:
    """Print bounded daemon log tails without reading configs or key files."""

    for index in range(PEER_COUNT):
        path = target / f"peer{index}.log"
        if not path.is_file() or path.is_symlink():
            continue
        try:
            with path.open("rb") as stream:
                stream.seek(0, os.SEEK_END)
                start = max(0, stream.tell() - MAX_LOG_TAIL_BYTES)
                stream.seek(start)
                payload = stream.read(MAX_LOG_TAIL_BYTES)
        except OSError as error:
            print(f"\n--- cannot read {path}: {error} ---", file=sys.stderr)
            continue
        if start:
            _, separator, payload = payload.partition(b"\n")
            if not separator:
                payload = b""
        lines = payload.decode("utf-8", errors="replace").splitlines()[-40:]
        print(f"\n--- {path} (last {len(lines)} lines) ---", file=sys.stderr)
        print("\n".join(lines), file=sys.stderr)


def up(
    args: argparse.Namespace,
    *,
    run: Runner = run_command,
    request: Request = http_request,
) -> dict[str, Any]:
    """Replace the disposable network and prove one signed transaction finalizes."""

    root = managed_root(args.dir, create=True)
    target = network_dir(root)
    if target.is_symlink():
        fail(f"refusing symlinked network directory: {target}")
    kagami, irohad, iroha = binary_paths(args, run)
    preflight_cli_surfaces(
        kagami,
        irohad,
        iroha,
        run,
        full_doctor=args.full_doctor,
    )
    target = reset_network(root, run)
    roots = torii_roots(args.base_api_port)
    try:
        print("Generating a fresh four-validator Taira network...", flush=True)
        generate_network(
            target,
            kagami,
            args.base_api_port,
            args.base_p2p_port,
            args.block_cadence_ms,
            run,
        )
        apply_canonical_taira_storage_profiles(target)
        validate_configs(target, irohad, run)
        require_bundle_identity(target, roots)
        env = os.environ.copy()
        env.update(
            {
                "IROHAD_BIN": str(irohad),
                "IROHA_CLI": str(iroha),
                # The generated localnet start script can maintain a long-lived
                # faucet reserve. A disposable deployment owns no predecessor
                # state, so that retry loop only delays the authoritative smoke.
                "IROHA_LOCALNET_FAUCET_RESERVE_RETRIES": "0",
            }
        )
        run(
            ["/bin/bash", str(target / "start.sh")],
            cwd=target,
            env=env,
            timeout=60,
            capture_output=False,
        )
        require_running_cohort(target, run)
        # Health/readiness can become available before genesis is committed.
        # Do not quote or submit a signed transaction against the empty height-0
        # state, where the freshly generated authority is not registered yet.
        baseline = wait_for_cluster(roots, args.timeout_seconds, request, above=0)
        print(
            f"Four validators converged at height {baseline[0]}; submitting signed smoke...",
            flush=True,
        )
        submitted = run(
            [
                str(iroha),
                "--machine",
                "-c",
                str(target / "client.toml"),
                "--fee-payer",
                "authority",
                "--output-format",
                "json",
                "tx",
                "ping",
                "--no-wait",
                "--log-level",
                "INFO",
                "--msg",
                "taira-devnet-ready",
            ],
            cwd=target,
            timeout=args.timeout_seconds,
        )
        transaction_hash = submitted_transaction_hash(submitted)
        print(
            f"Submitted {transaction_hash}; waiting for typed Applied status...",
            flush=True,
        )
        waited = run(
            [
                str(iroha),
                "--machine",
                "-c",
                str(target / "client.toml"),
                "--output-format",
                "json",
                "tx",
                "status",
                "--hash",
                transaction_hash,
                "--wait",
                "--timeout-ms",
                str(max(1, int(args.timeout_seconds * 1000))),
                "--poll-interval-ms",
                "250",
                "--terminal-status",
                "applied",
            ],
            cwd=target,
            timeout=args.timeout_seconds + 5,
        )
        require_applied_transaction(waited, transaction_hash)
        print("Signed smoke reached Applied; waiting for four-peer convergence...", flush=True)
        final = wait_for_cluster(roots, args.timeout_seconds, request, above=max(baseline))
        check_all_mcp(roots, request)
        if args.full_doctor:
            run_full_doctor(target, iroha, roots[0], run)
    except (DevnetError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        stop_network(root, run, tolerate_failure=True)
        dump_logs(target)
        if isinstance(error, subprocess.TimeoutExpired):
            fail(f"command timed out: {error.cmd}")
        if isinstance(error, KeyboardInterrupt):
            fail("Taira devnet startup was interrupted; the generated cohort was stopped")
        raise
    report = {
        "directory": str(target),
        "client_config": str(target / "client.toml"),
        "torii_roots": list(roots),
        "baseline_height": baseline[0],
        "final_height": final[0],
        "transaction_hash": transaction_hash,
        "terminal_status": "Applied",
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


def check(
    args: argparse.Namespace,
    *,
    run: Runner = run_command,
    request: Request = http_request,
) -> dict[str, Any]:
    """Read readiness and convergence without submitting a transaction."""

    root = managed_root(args.dir, create=False)
    target = require_network_bundle(root)
    roots = (
        bundle_torii_roots(target)
        if args.base_api_port is None
        else torii_roots(args.base_api_port)
    )
    require_canonical_taira_storage_profiles(target)
    require_bundle_identity(target, roots)
    require_running_cohort(target, run)
    heights = wait_for_cluster(roots, args.timeout_seconds, request)
    check_all_mcp(roots, request)
    if args.full_doctor:
        iroha = require_executable(args.iroha.expanduser().absolute())
        run_full_doctor(target, iroha, roots[0], run)
    report = {"directory": str(target), "torii_roots": list(roots), "height": heights[0]}
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


def down(args: argparse.Namespace, *, run: Runner = run_command) -> dict[str, Any]:
    """Stop the peers and destroy their disposable runtime signer keys."""

    root = managed_root(args.dir, create=False)
    target = require_stoppable_network(root)
    stop_network(root, run)
    delete_runtime_signer_files(target)
    report = {"directory": str(target), "runtime_signers_deleted": True, "stopped": True}
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument(
        "--dir", type=Path, default=DEFAULT_DIR, help="managed disposable directory"
    )
    commands = result.add_subparsers(dest="command", required=True)

    up_parser = commands.add_parser("up", help="replace, start, and verify the devnet")
    up_parser.add_argument("--profile", default="local-release", help="Cargo profile")
    up_parser.add_argument("--target-dir", type=Path, default=REPO_ROOT / "target")
    up_parser.add_argument(
        "--bin-dir",
        type=Path,
        help="directory containing Kagami, the Taira daemon, and the Iroha CLI",
    )
    up_parser.add_argument(
        "--no-build", action="store_true", help="use binaries already in --bin-dir"
    )
    up_parser.add_argument(
        "--build-timeout-seconds",
        type=float,
        help="optional Cargo build deadline; the default lets a cold build finish",
    )
    up_parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=DEFAULT_OPERATION_TIMEOUT_SECONDS,
        help="deadline for each startup, transaction, and convergence phase",
    )
    up_parser.add_argument("--base-api-port", type=int, default=DEFAULT_API_PORT)
    up_parser.add_argument("--base-p2p-port", type=int, default=DEFAULT_P2P_PORT)
    up_parser.add_argument(
        "--block-cadence-ms",
        type=int,
        default=DEFAULT_BLOCK_CADENCE_MS,
        help="signed cadence used to derive robust local consensus deadlines",
    )
    up_parser.add_argument(
        "--full-doctor",
        action="store_true",
        help="also require the broad public Taira product surface",
    )
    up_parser.set_defaults(handler=up)

    check_parser = commands.add_parser("check", help="read four-peer readiness and height")
    check_parser.add_argument("--timeout-seconds", type=float, default=20)
    check_parser.add_argument(
        "--base-api-port",
        type=int,
        help="override the generated bundle's Torii base port",
    )
    check_parser.add_argument("--full-doctor", action="store_true")
    check_parser.add_argument(
        "--iroha", type=Path, default=REPO_ROOT / "target/local-release/iroha"
    )
    check_parser.set_defaults(handler=check)

    down_parser = commands.add_parser("down", help="stop the disposable peers and retain logs")
    down_parser.set_defaults(handler=down)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    """Run the selected disposable devnet operation."""

    args = parser().parse_args(argv)
    try:
        args.handler(args)
    except DevnetError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
