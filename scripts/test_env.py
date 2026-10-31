#!/usr/bin/env python3
"""Run a disposable four-validator network for the Python test suites.

Requires Python 3.10+ and built `iroha3d`, `iroha` and `kagami` binaries: run
`cargo build -p irohad -p iroha_cli -p iroha_kagami` first, or point BIN_IROHAD, BIN_IROHA and
BIN_KAGAMI at existing ones.

`setup` generates a bundle with `kagami localnet generate` under --out-dir (default `test/`),
starts it and waits until every Torii endpoint answers. `cleanup` stops those validators and
removes the directory, and refuses a directory this script did not create.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from typing import Mapping
from urllib.error import URLError
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT_DIR = ROOT / "test"
MARKER = ".iroha-test-env"
BASE_API_PORT = 8080
PEERS = 4
READY_TIMEOUT_SECONDS = 180.0
# The CLI suite writes per-peer configs and instruction files under these directories.
SUITE_DIRECTORIES = ("peer_configs", "isi")


class TestEnvError(RuntimeError):
    """A setup or cleanup step that cannot continue."""


def binary_path(name: str, env_name: str, environ: Mapping[str, str] = os.environ) -> Path:
    """Select one binary from its environment override or the debug build directory."""
    selected = environ.get(env_name)
    target = Path(environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    path = Path(selected) if selected else target / "debug" / name
    if not path.is_absolute():
        path = ROOT / path
    if not path.is_file():
        raise TestEnvError(
            f"{name} not found at {path}; run `cargo build -p irohad -p iroha_cli -p iroha_kagami` "
            f"or set {env_name}"
        )
    return path


def generate_command(kagami: Path, out_dir: Path) -> list[str]:
    return [
        str(kagami),
        "localnet",
        "generate",
        "--out-dir",
        str(out_dir),
        "--peers",
        str(PEERS),
        "--bind-host",
        "127.0.0.1",
        "--public-host",
        "127.0.0.1",
        "--base-api-port",
        str(BASE_API_PORT),
    ]


def torii_ports() -> list[int]:
    return [BASE_API_PORT + peer for peer in range(PEERS)]


def healthy(port: int) -> bool:
    try:
        with urlopen(f"http://127.0.0.1:{port}/health", timeout=2) as response:
            return response.status == 200
    except (URLError, OSError):
        return False


def wait_until_ready(timeout: float = READY_TIMEOUT_SECONDS) -> None:
    deadline = time.monotonic() + timeout
    pending = torii_ports()
    while pending:
        pending = [port for port in pending if not healthy(port)]
        if pending and time.monotonic() >= deadline:
            raise TestEnvError(f"Torii did not answer on ports {pending} within {timeout:.0f}s")
        if pending:
            time.sleep(1)


def env_lines(out_dir: Path) -> list[str]:
    """The settings pytests/iroha_cli_tests reads from its .env file."""
    ports = torii_ports()
    return [
        f"TMP_DIR={out_dir}",
        "IROHA_CLI_BINARY=iroha",
        "IROHA_CLI_CONFIG=client.toml",
        f"TORII_API_PORT_MIN={ports[0]}",
        f"TORII_API_PORT_MAX={ports[-1]}",
    ]


def setup(out_dir: Path) -> None:
    if out_dir.exists():
        raise TestEnvError(f"{out_dir} already exists; run `scripts/test_env.py cleanup` first")
    daemon = binary_path("iroha3d", "BIN_IROHAD")
    client = binary_path("iroha", "BIN_IROHA")
    kagami = binary_path("kagami", "BIN_KAGAMI")
    subprocess.run(generate_command(kagami, out_dir), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    (out_dir / MARKER).touch()
    for directory in SUITE_DIRECTORIES:
        (out_dir / directory).mkdir()
    shutil.copy2(client, out_dir / "iroha")
    subprocess.run(
        ["bash", "start.sh"],
        cwd=out_dir,
        check=True,
        env={**os.environ, "IROHAD_BIN": str(daemon)},
    )
    try:
        wait_until_ready()
    except TestEnvError:
        cleanup(out_dir)
        raise
    print("\n".join(env_lines(out_dir)))


def cleanup(out_dir: Path) -> None:
    if not out_dir.exists():
        return
    if not (out_dir / MARKER).exists():
        raise TestEnvError(f"{out_dir} was not created by this script; refusing to remove it")
    subprocess.run(["bash", "stop.sh"], cwd=out_dir, check=False)
    shutil.rmtree(out_dir)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("setup", "cleanup"))
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR)
    args = parser.parse_args(argv)
    out_dir = args.out_dir if args.out_dir.is_absolute() else ROOT / args.out_dir
    try:
        if args.action == "setup":
            setup(out_dir)
        else:
            cleanup(out_dir)
    except (TestEnvError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
