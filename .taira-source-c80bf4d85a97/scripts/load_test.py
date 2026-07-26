#!/usr/bin/env python3
"""Drive a local Iroha network with ping transactions until they commit."""

from __future__ import annotations

import json
import subprocess
import sys
import time
from typing import Any


API_URL = "http://127.0.0.1:8080"
CONFIG = "/tmp/iroha-localnet-7peer/client.toml"
BINARY = "target/debug/iroha"
STATUS_TIMEOUT_SECONDS = 5.0


def get_status(*, timeout_seconds: float = STATUS_TIMEOUT_SECONDS) -> dict[str, Any] | None:
    """Return the current status object, or ``None`` when it cannot be read."""

    if timeout_seconds <= 0:
        return None
    request_timeout = min(STATUS_TIMEOUT_SECONDS, timeout_seconds)
    try:
        result = subprocess.run(
            ["curl", "-s", "--max-time", str(request_timeout), f"{API_URL}/status"],
            capture_output=True,
            text=True,
            check=False,
            timeout=request_timeout,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode != 0 or not result.stdout.strip():
        return None
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return payload if isinstance(payload, dict) else None


def send_ping(*, timeout_seconds: float) -> bool:
    """Submit one ping transaction and report whether it was confirmed."""

    message = f"load-{int(time.time() * 1000)}"
    try:
        result = subprocess.run(
            [
                BINARY,
                "--config",
                CONFIG,
                "transaction",
                "ping",
                "--msg",
                message,
                "--count",
                "1",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return result.returncode == 0


def _counter(status: dict[str, Any] | None, key: str) -> int | None:
    if status is None:
        return None
    value = status.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        return None
    return value


def run_load_test(
    *,
    ready_attempts: int = 60,
    target_height: int = 100,
    timeout_seconds: float = 300,
    poll_seconds: float = 0.5,
) -> int:
    """Run the load loop and return a process-style status code."""

    print("Waiting for network...")
    ready = False
    for _ in range(ready_attempts):
        status = get_status()
        height = _counter(status, "blocks")
        if height is not None:
            print(f"Network ready. Blocks: {height}")
            ready = True
            break
        time.sleep(1)

    if not ready:
        print("Network failed to start.")
        return 1

    print("Starting load...")
    start_time = time.monotonic()
    deadline = start_time + timeout_seconds
    confirmed_submissions = 0
    failed_submissions = 0
    while True:
        now = time.monotonic()
        elapsed = now - start_time
        if now >= deadline:
            print(
                "Timeout reached "
                f"({confirmed_submissions} confirmed, {failed_submissions} failed submissions)."
            )
            return 1

        status = get_status(timeout_seconds=deadline - now)
        now = time.monotonic()
        elapsed = now - start_time
        if now >= deadline:
            print(
                "Timeout reached "
                f"({confirmed_submissions} confirmed, {failed_submissions} failed submissions)."
            )
            return 1
        height = _counter(status, "blocks") or 0
        print(f"Height: {height}, Elapsed: {elapsed:.1f}s")

        if height >= target_height and confirmed_submissions > 0:
            print(
                f"Success! Reached height {height} after "
                f"{confirmed_submissions} confirmed ping transaction(s) in {elapsed:.1f}s"
            )
            return 0

        if send_ping(timeout_seconds=deadline - now):
            confirmed_submissions += 1
        else:
            failed_submissions += 1
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(max(poll_seconds, 0), remaining))


def main() -> int:
    """Run the default local-network load test."""

    return run_load_test()


if __name__ == "__main__":
    sys.exit(main())
