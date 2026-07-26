#!/usr/bin/env python3
"""Run Python SDK integration tests against a live Torii node.

The harness optionally spins up the single-node docker compose topology, waits
for Torii to answer `/status`, and then invokes `pytest` with the integration
marker. Pass `--no-start` to target an already running node.
"""

from __future__ import annotations

import argparse
import atexit
import os
import pathlib
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from typing import Iterable, List, Sequence

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_COMPOSE_FILE = REPO_ROOT / "defaults" / "docker-compose.single.yml"
DEFAULT_TORII_URL = "http://127.0.0.1:8080"
DEFAULT_SERVICE = "irohad0"
DEFAULT_WAIT_SECONDS = 90
DEFAULT_PYTEST_PATH = "python/iroha_python/tests/integration"


def _parse_args(argv: Sequence[str]) -> argparse.Namespace:
    start_env = os.environ.get("START_TORII")
    start_default = True
    if start_env is not None:
        start_default = start_env.strip() not in {"0", "false", "False"}

    compose_file_default = os.environ.get("COMPOSE_FILE")
    if compose_file_default:
        compose_file_default = pathlib.Path(compose_file_default)
    else:
        compose_file_default = DEFAULT_COMPOSE_FILE

    service_default = os.environ.get("COMPOSE_SERVICE", DEFAULT_SERVICE)
    wait_env = os.environ.get("WAIT_SECONDS")
    if wait_env is not None:
        try:
            wait_default = int(wait_env)
        except ValueError:
            wait_default = DEFAULT_WAIT_SECONDS
    else:
        wait_default = DEFAULT_WAIT_SECONDS
    pytest_path_default = os.environ.get("PYTEST_PATH", DEFAULT_PYTEST_PATH)
    compose_bin_default = os.environ.get("COMPOSE_BIN")

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--torii-url",
        default=os.environ.get("IROHA_TORII_URL", DEFAULT_TORII_URL),
        help="Torii base URL used by the integration tests "
        f"(default: %(default)s, overrides IROHA_TORII_URL).",
    )
    parser.add_argument(
        "--compose-file",
        type=pathlib.Path,
        default=compose_file_default,
        help=f"Docker compose file to start (default: {DEFAULT_COMPOSE_FILE}).",
    )
    parser.add_argument(
        "--service",
        default=service_default,
        help=f"Compose service name to start (default: {DEFAULT_SERVICE}).",
    )
    parser.add_argument(
        "--wait-seconds",
        type=int,
        default=wait_default,
        help=f"Maximum seconds to wait for Torii readiness (default: {DEFAULT_WAIT_SECONDS}).",
    )
    parser.add_argument(
        "--pytest-path",
        default=pytest_path_default,
        help=f"Pytest target (default: {DEFAULT_PYTEST_PATH}).",
    )
    parser.add_argument(
        "--compose-bin",
        default=compose_bin_default,
        help="Explicit compose command (e.g. 'docker compose' or 'docker-compose'). "
        "When unset the harness auto-detects the docker compose plugin or docker-compose binary.",
    )
    parser.add_argument(
        "--no-start",
        dest="start",
        action="store_false",
        help="Skip docker compose startup and reuse an existing Torii node.",
    )
    parser.set_defaults(start=start_default)
    parser.add_argument(
        "pytest_args",
        nargs=argparse.REMAINDER,
        help="Additional arguments forwarded to pytest (prefix with '--' to separate).",
    )
    return parser.parse_args(list(argv))


def _split_compose_bin(value: str) -> List[str]:
    parts = value.split()
    if not parts:
        raise ValueError("compose command must not be empty")
    return parts


def _read_compose_env(env_var: str) -> list[str] | None:
    override = os.environ.get(env_var)
    if not override:
        return None
    return _split_compose_bin(override)


def _find_compose_binary(explicit: str | None = None) -> List[str]:
    """Return the docker compose invocation as a list."""

    if explicit:
        return _split_compose_bin(explicit)

    override = _read_compose_env("PYTHON_TORII_COMPOSE_BIN")
    if override:
        return override

    candidates: Iterable[List[str]] = (
        ["docker", "compose"],
        ["docker-compose"],
    )

    for candidate in candidates:
        binary = candidate[0]
        if shutil.which(binary) is None:
            continue
        try:
            subprocess.run(
                candidate + ["version"],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except (subprocess.CalledProcessError, FileNotFoundError):
            continue
        return list(candidate)

    raise RuntimeError(
        "docker compose plugin or docker-compose binary is required; "
        "install Docker or set PYTHON_TORII_COMPOSE_BIN"
    )


def _run_compose(compose_cmd: Sequence[str], args: Sequence[str]) -> None:
    cmd = list(compose_cmd) + list(args)
    subprocess.run(cmd, check=True)


def _wait_for_torii(url: str, timeout_seconds: int) -> None:
    deadline = time.monotonic() + timeout_seconds
    status_url = f"{url.rstrip('/')}/status"

    while True:
        try:
            with urllib.request.urlopen(status_url, timeout=5) as response:
                if 200 <= response.getcode() < 300:
                    return
        except (urllib.error.URLError, urllib.error.HTTPError, OSError):
            pass

        if time.monotonic() >= deadline:
            raise RuntimeError(f"Timed out waiting for Torii at {status_url}")
        time.sleep(2)


def _run_pytest(pytest_path: str, pytest_args: Sequence[str], env: dict[str, str]) -> int:
    extra = list(pytest_args)
    if extra and extra[0] == "--":
        extra = extra[1:]
    if not extra:
        extra = ["-m", "integration", "-q"]
    cmd = [sys.executable, "-m", "pytest", pytest_path] + extra
    completed = subprocess.run(cmd, env=env)
    return completed.returncode


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv or sys.argv[1:])
    compose_cmd: List[str] | None = None
    compose_file = args.compose_file
    if args.start and not compose_file.exists():
        raise FileNotFoundError(f"compose file not found: {compose_file}")

    if args.start:
        compose_cmd = _find_compose_binary(args.compose_bin)
        compose_args = ["-f", str(compose_file), "up", "-d", args.service]
        _run_compose(compose_cmd, compose_args)

        def _cleanup() -> None:
            try:
                _run_compose(compose_cmd, ["-f", str(compose_file), "down", "--remove-orphans"])
            except Exception as exc:  # pragma: no cover - best-effort cleanup
                print(f"[integration] warning: failed to shut down compose stack: {exc}", file=sys.stderr)

        atexit.register(_cleanup)
        _wait_for_torii(args.torii_url, args.wait_seconds)

    env = os.environ.copy()
    env["IROHA_TORII_URL"] = args.torii_url
    return _run_pytest(args.pytest_path, args.pytest_args, env)


if __name__ == "__main__":
    sys.exit(main())
