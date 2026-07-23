"""Regression tests for ``scripts/load_test.py``."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "load_test.py"
SPEC = importlib.util.spec_from_file_location("iroha_load_test", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
LOAD_TEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOAD_TEST)


def _install_statuses(
    monkeypatch: pytest.MonkeyPatch,
    statuses: list[dict[str, int]],
) -> None:
    iterator = iter(statuses)
    monkeypatch.setattr(LOAD_TEST, "get_status", lambda **_kwargs: next(iterator))
    monkeypatch.setattr(LOAD_TEST.time, "sleep", lambda _seconds: None)


def test_rising_block_height_does_not_hide_failed_submissions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_statuses(
        monkeypatch,
        [
            {"blocks": 1, "txs_approved": 10},
            {"blocks": 100, "txs_approved": 10},
        ],
    )
    clock = [0.0]
    monkeypatch.setattr(LOAD_TEST.time, "monotonic", lambda: clock[0])

    def fail_ping(**_kwargs: object) -> bool:
        clock[0] = 1.0
        return False

    monkeypatch.setattr(LOAD_TEST, "send_ping", fail_ping)

    assert LOAD_TEST.run_load_test(timeout_seconds=0.5, poll_seconds=0) == 1


def test_success_requires_a_confirmed_ping_submission(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_statuses(
        monkeypatch,
        [
            {"blocks": 1},
            {"blocks": 100},
            {"blocks": 101},
        ],
    )
    monkeypatch.setattr(LOAD_TEST.time, "monotonic", lambda: 0.0)
    monkeypatch.setattr(LOAD_TEST, "send_ping", lambda **_kwargs: True)

    assert LOAD_TEST.run_load_test(timeout_seconds=1, poll_seconds=0) == 0


def test_ping_command_waits_for_confirmation(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: list[str] = []
    captured_timeout: list[float] = []

    class Result:
        returncode = 0

    def run(command: list[str], **kwargs: object) -> Result:
        captured.extend(command)
        captured_timeout.append(float(kwargs["timeout"]))
        return Result()

    monkeypatch.setattr(LOAD_TEST.subprocess, "run", run)

    assert LOAD_TEST.send_ping(timeout_seconds=12.5)
    assert "--no-wait" not in captured
    assert captured_timeout == [12.5]


def test_ping_timeout_is_reported_as_failed_submission(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def time_out(*_args: object, **_kwargs: object) -> object:
        raise LOAD_TEST.subprocess.TimeoutExpired("iroha", 0.1)

    monkeypatch.setattr(LOAD_TEST.subprocess, "run", time_out)

    assert not LOAD_TEST.send_ping(timeout_seconds=0.1)


def test_status_timeout_is_reported_as_unavailable(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def time_out(*_args: object, **_kwargs: object) -> object:
        raise LOAD_TEST.subprocess.TimeoutExpired("curl", 0.1)

    monkeypatch.setattr(LOAD_TEST.subprocess, "run", time_out)

    assert LOAD_TEST.get_status() is None


def test_status_poll_cannot_consume_deadline_then_launch_ping(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    clock = [0.0]
    status_calls = 0

    def get_status(**_kwargs: object) -> dict[str, int]:
        nonlocal status_calls
        status_calls += 1
        if status_calls == 2:
            clock[0] = 1.0
        return {"blocks": 100}

    monkeypatch.setattr(LOAD_TEST, "get_status", get_status)
    monkeypatch.setattr(LOAD_TEST.time, "monotonic", lambda: clock[0])
    monkeypatch.setattr(LOAD_TEST.time, "sleep", lambda _seconds: None)

    def unexpected_ping(**_kwargs: object) -> bool:
        raise AssertionError("ping must not launch after the deadline")

    monkeypatch.setattr(LOAD_TEST, "send_ping", unexpected_ping)

    assert LOAD_TEST.run_load_test(timeout_seconds=1, poll_seconds=10) == 1


def test_poll_sleep_is_capped_to_remaining_deadline(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    clock = [0.0]
    sleeps: list[float] = []
    monkeypatch.setattr(LOAD_TEST, "get_status", lambda **_kwargs: {"blocks": 100})
    monkeypatch.setattr(LOAD_TEST.time, "monotonic", lambda: clock[0])

    def fail_ping(**_kwargs: object) -> bool:
        clock[0] = 0.75
        return False

    def sleep(seconds: float) -> None:
        sleeps.append(seconds)
        clock[0] += seconds

    monkeypatch.setattr(LOAD_TEST, "send_ping", fail_ping)
    monkeypatch.setattr(LOAD_TEST.time, "sleep", sleep)

    assert LOAD_TEST.run_load_test(timeout_seconds=1, poll_seconds=10) == 1
    assert sleeps == [0.25]
