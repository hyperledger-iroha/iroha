"""Unit tests for the isolated Cargo build profiler."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "profile_build.py"
SPEC = importlib.util.spec_from_file_location("profile_build", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_validate_target_dir_refuses_implicit_cache_reuse(tmp_path: Path) -> None:
    target = tmp_path / "profile-target"
    target.mkdir()
    (target / "artifact").write_bytes(b"artifact")

    with pytest.raises(ValueError, match="pass --reuse"):
        MODULE.validate_target_dir(target, reuse=False)
    assert MODULE.validate_target_dir(target, reuse=True) == target.resolve()


def test_validate_target_dir_creates_isolated_directory(tmp_path: Path) -> None:
    target = tmp_path / "new" / "target"
    resolved = MODULE.validate_target_dir(target, reuse=False)
    assert resolved.is_dir()
    assert not any(resolved.iterdir())


def test_directory_size_ignores_symlinks(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    (target / "artifact").write_bytes(b"1234")
    (target / "artifact-link").symlink_to(target / "artifact")

    assert MODULE.directory_size(target) == 4


def test_process_table_parser_normalizes_rss_to_bytes() -> None:
    assert MODULE.parse_process_table(" 10  1  4\n11 10 8\nheader noise\n") == {
        10: (1, 4 * 1024),
        11: (10, 8 * 1024),
    }


def test_parse_args_requires_positive_jobs_during_measurement(tmp_path: Path) -> None:
    args = MODULE.parse_args(
        ["workspace", "--target-dir", str(tmp_path), "--jobs", "0"]
    )
    with pytest.raises(ValueError, match="greater than zero"):
        MODULE.measure(tmp_path, args.scenario, tmp_path, args.jobs)


def test_measure_excludes_repeated_ps_sampler_cpu(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class Process:
        pid = 4242

        def __init__(self) -> None:
            self._polls = iter((None, None, 0))

        def poll(self) -> int | None:
            return next(self._polls)

        def wait(self) -> int:
            return 0

    cpu_samples = iter(
        (
            (100.0, 50.0),
            (100.0, 50.0),
            (100.1, 50.05),
            (100.1, 50.05),
            (100.3, 50.15),
            (100.3, 50.15),
            (100.6, 50.3),
            (105.6, 52.3),
        )
    )
    rss_samples = iter((100, 300, 200))
    sleeps: list[float] = []
    monotonic = iter((10.0, 12.0))

    monkeypatch.setattr(MODULE.subprocess, "Popen", lambda *_args, **_kwargs: Process())
    monkeypatch.setattr(MODULE, "_child_cpu_seconds", lambda: next(cpu_samples))
    monkeypatch.setattr(
        MODULE, "process_tree_rss_bytes", lambda _pid: next(rss_samples)
    )
    monkeypatch.setattr(MODULE.time, "sleep", sleeps.append)
    monkeypatch.setattr(MODULE.time, "monotonic", lambda: next(monotonic))

    measurement = MODULE.measure(tmp_path, "data-model", tmp_path, None)

    assert measurement.user_cpu_seconds == pytest.approx(5.0)
    assert measurement.system_cpu_seconds == pytest.approx(2.0)
    assert measurement.peak_process_tree_rss_bytes == 300
    assert sleeps == [0.25, 0.25]


def test_measure_attempts_rss_sample_before_first_poll(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class ImmediatelyCompleteProcess:
        pid = 8675

        @staticmethod
        def poll() -> int:
            return 0

        @staticmethod
        def wait() -> int:
            return 0

    cpu_samples = iter(((1.0, 2.0), (1.0, 2.0), (1.0, 2.0), (1.0, 2.0)))
    sampled_pids: list[int] = []
    sleeps: list[float] = []
    monotonic = iter((5.0, 5.01))

    monkeypatch.setattr(
        MODULE.subprocess,
        "Popen",
        lambda *_args, **_kwargs: ImmediatelyCompleteProcess(),
    )
    monkeypatch.setattr(MODULE, "_child_cpu_seconds", lambda: next(cpu_samples))
    monkeypatch.setattr(
        MODULE,
        "process_tree_rss_bytes",
        lambda pid: sampled_pids.append(pid) or 4096,
    )
    monkeypatch.setattr(MODULE.time, "sleep", sleeps.append)
    monkeypatch.setattr(MODULE.time, "monotonic", lambda: next(monotonic))

    measurement = MODULE.measure(tmp_path, "cli", tmp_path, None)

    assert sampled_pids == [ImmediatelyCompleteProcess.pid]
    assert measurement.peak_process_tree_rss_bytes == 4096
    assert sleeps == []
