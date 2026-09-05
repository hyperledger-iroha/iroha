from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import tomllib

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_kagemusha_real_proof_guarded.py"
SPEC = importlib.util.spec_from_file_location("run_kagemusha_real_proof_guarded", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["run_kagemusha_real_proof_guarded"] = MODULE
SPEC.loader.exec_module(MODULE)


def test_command_runs_only_the_dedicated_real_mint_proof_binary(tmp_path: Path) -> None:
    command = MODULE.proof_command(tmp_path)
    assert command[:7] == [
        "cargo",
        "iroha-fast",
        "--zero-debug",
        "--no-sccache",
        "--target-dir",
        str(tmp_path),
        "--",
    ]
    assert command.count(MODULE.PROOF_BINARY) == 1
    assert command[-2:] == [
        "--features",
        MODULE.PROOF_FEATURE,
    ]
    assert "--locked" in command
    assert "run" in command
    assert "--bin" in command
    assert "--lib" not in command
    assert MODULE.TEST_NAME not in command


def test_proof_binary_is_an_explicit_non_default_target() -> None:
    repository = MODULE_PATH.parents[1]
    with (repository / "crates/iroha_core/Cargo.toml").open("rb") as source:
        manifest = tomllib.load(source)
    targets = {target["name"]: target for target in manifest["bin"]}
    assert targets[MODULE.PROOF_BINARY] == {
        "name": MODULE.PROOF_BINARY,
        "path": "src/bin/kagemusha_real_proof.rs",
        "required-features": [MODULE.PROOF_FEATURE],
    }
    assert manifest["features"][MODULE.PROOF_FEATURE] == ["zk-halo2-ipa"]
    assert MODULE.PROOF_FEATURE not in manifest["features"]["default"]


def test_default_target_uses_the_dedicated_proof_memory_lane() -> None:
    args = MODULE._parser([])
    assert args.target_dir.name == "proof-memory-fix"
    assert args.target_dir.parent.name == ".taira-testnet-build-targets"


def test_memory_sample_enforces_larger_of_rss_and_footprint() -> None:
    rss_larger = MODULE.MemorySample(20, 10, 2)
    footprint_larger = MODULE.MemorySample(10, 30, 3)
    assert rss_larger.enforced_bytes == 20
    assert footprint_larger.enforced_bytes == 30


def test_guard_samples_frequently_enough_to_stop_runaway_allocations() -> None:
    assert MODULE.DEFAULT_SAMPLE_INTERVAL_SECONDS == 0.05


def test_proof_environment_removes_compiler_wrappers_and_sccache_configuration(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    hostile = {
        "RUSTC_WRAPPER": "/tmp/sccache",
        "RUSTC_WORKSPACE_WRAPPER": "/tmp/workspace-wrapper",
        "CARGO_BUILD_RUSTC_WRAPPER": "/tmp/cargo-wrapper",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "/tmp/cargo-workspace-wrapper",
        "SCCACHE_DIR": "/tmp/out-of-group-cache",
        "SCCACHE_ENDPOINT": "https://cache.invalid",
        "SCCACHE_SERVER_PORT": "1234",
    }
    for name, value in hostile.items():
        monkeypatch.setenv(name, value)
    monkeypatch.setenv("PROOF_GUARD_RETAINED", "yes")

    environment = MODULE._proof_environment(tmp_path)

    assert environment["RUSTC_WRAPPER"] == ""
    assert environment["RUSTC_WORKSPACE_WRAPPER"] == ""
    assert "CARGO_BUILD_RUSTC_WRAPPER" not in environment
    assert "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER" not in environment
    assert not any(name.startswith("SCCACHE_") for name in environment)
    assert environment["CARGO_TARGET_DIR"] == str(tmp_path)
    assert environment["TAIRA_TESTNET_CARGO_TARGET_DIR"] == str(tmp_path)
    assert environment["PROOF_GUARD_RETAINED"] == "yes"


def test_accounting_tolerates_normal_process_group_churn() -> None:
    class ChurningAccounting:
        def __init__(self) -> None:
            self.snapshots = iter([(500,), (500, 501), (500, 501), (500, 501)])

        def _process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return next(self.snapshots)

        def _identity(self, process_id: int, process_group_id: int) -> None:
            assert process_id in (500, 501)
            assert process_group_id == 500

        def _memory(self, process_id: int) -> tuple[int, int]:
            return (process_id, process_id * 2)

    sample = MODULE.DarwinProcessAccounting.sample(ChurningAccounting(), 500)
    assert sample == MODULE.MemorySample(1_001, 2_002, 2)


def test_accounting_accepts_an_authenticated_snapshot_when_a_child_exits() -> None:
    class ExitingChildAccounting:
        def __init__(self) -> None:
            self.snapshots = iter([(500, 501), (500,)])

        def _process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return next(self.snapshots)

        def _identity(self, process_id: int, process_group_id: int) -> None:
            assert process_group_id == 500
            if process_id == 501:
                raise MODULE.ProcessRaced("compiler exited")

        def _memory(self, process_id: int) -> tuple[int, int]:
            assert process_id == 500
            return (700, 900)

    sample = MODULE.DarwinProcessAccounting.sample(ExitingChildAccounting(), 500)
    assert sample == MODULE.MemorySample(700, 900, 1)


def test_accounting_fails_closed_when_current_pids_never_become_accounted(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class PermanentlyChurningAccounting:
        def __init__(self) -> None:
            self.enumeration = 0

        def _process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            self.enumeration += 1
            return (500,) if self.enumeration % 2 else (500, 501)

        def _identity(self, process_id: int, process_group_id: int) -> None:
            assert process_id == 500
            assert process_group_id == 500

        def _memory(self, process_id: int) -> tuple[int, int]:
            assert process_id == 500
            return (700, 900)

    monkeypatch.setattr(MODULE, "STABLE_SNAPSHOT_ATTEMPTS", 3)
    with pytest.raises(MODULE.GuardError, match=r"current unaccounted PIDs: 501"):
        MODULE.DarwinProcessAccounting.sample(PermanentlyChurningAccounting(), 500)


def test_parser_rejects_ceiling_above_fixed_fail_safe() -> None:
    with pytest.raises(SystemExit):
        MODULE._parser(["--memory-limit-gib", "33"])


def test_runner_lock_is_nonblocking_and_owner_private(tmp_path: Path) -> None:
    path = tmp_path / "guard.lock"
    with MODULE._exclusive_lock(path, "test guard"):
        assert path.stat().st_mode & 0o777 == 0o600
        with pytest.raises(MODULE.LockUnavailable):
            with MODULE._exclusive_lock(path, "test guard"):
                raise AssertionError("contended lock unexpectedly acquired")


def test_external_target_is_required(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    repository = tmp_path / "iroha"
    repository.mkdir()
    (repository / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    args = MODULE._parser(
        [
            "--repository",
            str(repository),
            "--target-dir",
            str(repository / "target"),
        ]
    )
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    monkeypatch.setattr(MODULE, "_host_memory_bytes", lambda: 128 * 1024**3)
    with pytest.raises(MODULE.GuardError, match="outside the repository"):
        MODULE._validate_inputs(args)


def test_host_fraction_rejects_unsafe_limit(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    repository = tmp_path / "iroha"
    repository.mkdir()
    (repository / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    args = MODULE._parser(
        [
            "--repository",
            str(repository),
            "--target-dir",
            str(tmp_path / "target"),
            "--memory-limit-gib",
            "24",
        ]
    )
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    monkeypatch.setattr(MODULE, "_host_memory_bytes", lambda: 64 * 1024**3)
    with pytest.raises(MODULE.GuardError, match="one quarter"):
        MODULE._validate_inputs(args)


def test_summary_is_owner_private_and_replaced(tmp_path: Path) -> None:
    path = tmp_path / "summary.json"
    MODULE._write_summary(path, {"value": 1})
    MODULE._write_summary(path, {"value": 2})
    assert path.stat().st_mode & 0o777 == 0o600
    assert '"value": 2' in path.read_text(encoding="utf-8")


def test_termination_signals_only_authenticated_group_members(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class FakeProcess:
        pid = 500
        returncode = None

        def poll(self) -> int | None:
            return self.returncode

        def wait(self, timeout: float) -> int:
            assert timeout == 5
            self.returncode = -MODULE.signal.SIGTERM
            return self.returncode

        def kill(self) -> None:
            raise AssertionError("individual authenticated termination should complete")

    class FakeAccounting:
        def __init__(self) -> None:
            self.signals_sent = False

        def authenticated_process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return () if self.signals_sent else (500, 501, 502)

    process = FakeProcess()
    accounting = FakeAccounting()
    signals: list[tuple[int, MODULE.signal.Signals]] = []

    def fake_kill(process_id: int, signal_number: MODULE.signal.Signals) -> None:
        signals.append((process_id, signal_number))
        accounting.signals_sent = True

    monkeypatch.setattr(
        MODULE.os,
        "killpg",
        lambda _process_group_id, _signal_number: (_ for _ in ()).throw(PermissionError()),
    )
    monkeypatch.setattr(MODULE.os, "kill", fake_kill)
    MODULE._terminate_process_group(process, accounting)
    assert signals == [
        (502, MODULE.signal.SIGTERM),
        (501, MODULE.signal.SIGTERM),
        (500, MODULE.signal.SIGTERM),
    ]


def test_memory_limit_termination_kills_immediately(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class FakeProcess:
        pid = 500
        returncode = None

        def wait(self, timeout: float) -> int:
            assert timeout == 5
            self.returncode = -MODULE.signal.SIGKILL
            return self.returncode

        def kill(self) -> None:
            raise AssertionError("group SIGKILL should complete")

    class FakeAccounting:
        def __init__(self) -> None:
            self.members = iter(((500, 501), (), ()))

        def authenticated_process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return next(self.members)

    signals: list[tuple[int, MODULE.signal.Signals]] = []

    monkeypatch.setattr(
        MODULE.os,
        "killpg",
        lambda _process_group_id, _signal_number: (_ for _ in ()).throw(PermissionError()),
    )
    monkeypatch.setattr(
        MODULE.os,
        "kill",
        lambda process_id, signal_number: signals.append((process_id, signal_number)),
    )
    MODULE._terminate_process_group(FakeProcess(), FakeAccounting(), graceful=False)
    assert signals == [
        (501, MODULE.signal.SIGKILL),
        (500, MODULE.signal.SIGKILL),
    ]


def test_hard_kill_rescans_and_kills_a_member_spawned_during_termination(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class FakeProcess:
        pid = 500
        returncode = None

        def wait(self, timeout: float) -> int:
            assert timeout == 5
            self.returncode = -MODULE.signal.SIGKILL
            return self.returncode

        def kill(self) -> None:
            raise AssertionError("authenticated group termination should complete")

    class FakeAccounting:
        def __init__(self) -> None:
            self.members = iter(((500, 501), (502,), (), ()))

        def authenticated_process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return next(self.members)

    signals: list[tuple[int, MODULE.signal.Signals]] = []
    monkeypatch.setattr(MODULE, "TERMINATION_POLL_INTERVAL_SECONDS", 0.0)
    monkeypatch.setattr(
        MODULE.os,
        "killpg",
        lambda _process_group_id, _signal_number: (_ for _ in ()).throw(PermissionError()),
    )
    monkeypatch.setattr(
        MODULE.os,
        "kill",
        lambda process_id, signal_number: signals.append((process_id, signal_number)),
    )

    MODULE._terminate_process_group(FakeProcess(), FakeAccounting(), graceful=False)

    assert signals == [
        (501, MODULE.signal.SIGKILL),
        (500, MODULE.signal.SIGKILL),
        (502, MODULE.signal.SIGKILL),
    ]


def test_hard_kill_fails_closed_after_bounded_authenticated_attempts(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class FakeProcess:
        pid = 500

        def wait(self, timeout: float) -> int:
            raise AssertionError("must not wait while authenticated members survive")

    class FakeAccounting:
        def authenticated_process_ids(self, process_group_id: int) -> tuple[int, ...]:
            assert process_group_id == 500
            return (500,)

    signals: list[tuple[int, MODULE.signal.Signals]] = []
    monkeypatch.setattr(MODULE, "HARD_KILL_MAX_ATTEMPTS", 3)
    monkeypatch.setattr(MODULE, "HARD_KILL_TIMEOUT_SECONDS", 60.0)
    monkeypatch.setattr(MODULE, "TERMINATION_POLL_INTERVAL_SECONDS", 0.0)
    monkeypatch.setattr(
        MODULE.os,
        "killpg",
        lambda _process_group_id, _signal_number: (_ for _ in ()).throw(PermissionError()),
    )
    monkeypatch.setattr(
        MODULE.os,
        "kill",
        lambda process_id, signal_number: signals.append((process_id, signal_number)),
    )

    with pytest.raises(MODULE.GuardError, match="survived repeated authenticated SIGKILL"):
        MODULE._terminate_process_group(FakeProcess(), FakeAccounting(), graceful=False)

    assert signals == [(500, MODULE.signal.SIGKILL)] * 3
