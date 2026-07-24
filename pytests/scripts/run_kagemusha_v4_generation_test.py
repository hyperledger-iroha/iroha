"""Tests for the guarded Kagemusha V4 candidate-generation launcher."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import os
from pathlib import Path
import sys

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT_DIR / "scripts" / "run_kagemusha_v4_generation.py"
SPEC = importlib.util.spec_from_file_location("run_kagemusha_v4_generation", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


@pytest.fixture(autouse=True)
def _admit_test_output_filesystem(monkeypatch) -> None:
    """Keep tmp_path portable while filesystem-policy tests override explicitly."""

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "ext4")


def _fake_prebuilt_generator(tmp_path: Path, body: str = "") -> Path:
    executable = tmp_path / "kagemusha_recursive_spend_v4_bundle"
    executable.write_text(
        f"#!{sys.executable}\n"
        + """
import os
from pathlib import Path
import sys
if len(sys.argv) > 1 and sys.argv[1] in {"generate-candidate", "publish-staged-candidate"}:
    arguments = sys.argv[2:]
    out_dir = Path(arguments[arguments.index("--out-dir") + 1])
    staging_id = arguments[arguments.index("--staging-id") + 1]
    staging_name = arguments[arguments.index("--staging-name") + 1]
    parent_fd_text = arguments[arguments.index("--output-parent-fd") + 1]
    if not parent_fd_text.isascii() or not parent_fd_text.isdigit():
        raise SystemExit(92)
    parent_fd = int(parent_fd_text)
    parent_stat = os.fstat(parent_fd)
    executable_stat = os.stat(sys.argv[0])
    if staging_name != f".kagemusha-v4-staging-{staging_id}-work":
        raise SystemExit(93)
    staging_stat = os.stat(staging_name, dir_fd=parent_fd, follow_symlinks=False)
    if not Path(f"/dev/fd/{parent_fd}").is_dir() or not staging_stat.st_mode:
        raise SystemExit(94)
    observation_name = f"{staging_name}/fd-observations"
    flags = os.O_WRONLY | os.O_CREAT | os.O_APPEND
    observation = os.open(observation_name, flags, 0o600, dir_fd=parent_fd)
    try:
        os.write(
            observation,
            (
                f"{sys.argv[1]} {parent_stat.st_dev} {parent_stat.st_ino} "
                f"{executable_stat.st_dev} {executable_stat.st_ino} {sys.argv[0]}\\n"
            ).encode("ascii"),
        )
        os.fsync(observation)
    finally:
        os.close(observation)
    if sys.argv[1] == "publish-staged-candidate":
        os.rename(
            staging_name,
            out_dir.name,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
        )
        raise SystemExit(0)
"""
        + body
        + "",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    return executable


def _guarded_args(tmp_path: Path, executable: Path) -> list[str]:
    return [
        "--resource-report",
        str(tmp_path / "resource-report"),
        "--",
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
        "--source-commit",
        "0" * 40,
        "--source-tree-sha256",
        "1" * 64,
    ]


def test_effective_limit_is_half_physical_and_cannot_be_raised(monkeypatch) -> None:
    monkeypatch.setattr(MODULE, "_physical_memory_bytes", lambda: 12 * MODULE.BYTES_PER_GIB)

    assert MODULE._effective_memory_limit_bytes(None) == 256 * 1024 * 1024
    assert MODULE._effective_memory_limit_bytes(0.125) == 128 * 1024 * 1024
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(0.5)
    with pytest.raises(MODULE.resource_guard.GuardError, match="greater than zero"):
        MODULE._effective_memory_limit_bytes(float("nan"))


def test_runner_executes_small_owned_group_and_writes_reports(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    report_root = tmp_path / "resource-report"

    consume_capability = """
import os
fd = int(os.environ["IROHA_RESOURCE_GUARD_AUTH_FD"])
token = os.environ["IROHA_RESOURCE_GUARD_AUTH_TOKEN"]
record = os.read(fd, 256)
expected = f"IROHA_RESOURCE_GUARD_AUTH_V1:{token}\\n".encode("ascii")
if record != expected:
    raise SystemExit(91)
"""
    executable = _fake_prebuilt_generator(tmp_path, consume_capability)
    assert (
        MODULE.main(_guarded_args(tmp_path, executable))
        == 0
    )
    summary = json.loads(
        (report_root / "kagemusha_resource_summary.json").read_text(encoding="utf-8")
    )
    assert summary["exit_reason"] == "completed"
    assert summary["exit_status"] == 0
    assert summary["post_run_cleanup"] == "completed"
    assert summary["post_run_cleanup_removed"] == 1
    assert summary["post_run_validation"] == "completed"
    assert summary["post_success_finalize"] == "completed"
    assert summary["post_success_finalize_result"] == 1
    assert (tmp_path / "candidate").is_dir()
    assert summary["report_context"]["output_parent"]["canonical_path"] == str(
        tmp_path.resolve()
    )
    expected_parent = summary["report_context"]["output_parent"]
    executable_identity = summary["report_context"]["executable_identity"]
    observations = (tmp_path / "candidate" / "fd-observations").read_text(
        encoding="ascii"
    ).splitlines()
    assert len(observations) == 2
    execution_paths: list[str] = []
    for operation, observation in zip(
        ("generate-candidate", "publish-staged-candidate"), observations
    ):
        fields = observation.split()
        assert fields[:5] == [
            operation,
            str(expected_parent["device"]),
            str(expected_parent["inode"]),
            str(executable_identity["execution"]["file_device"]),
            str(executable_identity["execution"]["file_inode"]),
        ]
        assert len(fields) == 6
        execution_paths.append(fields[5])
    assert execution_paths[0] == execution_paths[1]
    assert execution_paths[0] == executable_identity["execution"]["canonical_path"]
    assert executable_identity["execution"]["method"] == "darwin_private_fd_copy"
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    assert executable_identity["canonical_path"] == str(executable.resolve())
    assert executable_identity["sha256"] == hashlib.sha256(
        executable.read_bytes()
    ).hexdigest()
    assert executable_identity["size_bytes"] == executable.stat().st_size
    assert 0 < summary["memory_limit_bytes"] <= MODULE.ABSOLUTE_MAX_MEMORY_BYTES
    assert summary["sample_interval_seconds"] == MODULE.SAMPLE_INTERVAL_SECONDS
    assert (report_root / "kagemusha_resource.jsonl").stat().st_size > 0


def test_runner_does_not_use_the_retired_boolean_supervision_marker() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")

    assert "IROHA_KAGEMUSHA_V4_RESOURCE_SUPERVISED" not in source
    assert "held_lock_descriptors=(heavy_lock, kagemusha_lock)" in source
    assert MODULE.ABSOLUTE_MAX_MEMORY_BYTES == 256 * 1024 * 1024
    assert MODULE.SAMPLE_INTERVAL_SECONDS == 0.05
    assert "sample_interval_seconds=SAMPLE_INTERVAL_SECONDS" in source


def test_runner_requires_prebuilt_generator_and_exact_subcommand(tmp_path: Path) -> None:
    report = tmp_path / "resource-report"
    assert MODULE.main(
        ["--resource-report", str(report), "--", "cargo", "run"]
    ) == 1
    assert not report.exists()

    executable = _fake_prebuilt_generator(tmp_path)
    assert MODULE.main(
        [
            "--resource-report",
            str(report),
            "--",
            str(executable),
            "--help",
            "--out-dir",
            str(tmp_path / "candidate"),
        ]
    ) == 1
    assert not report.exists()


def test_runner_refuses_to_overwrite_resource_evidence(tmp_path: Path) -> None:
    report_root = tmp_path / "resource-report"
    report_root.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1


def test_runner_refuses_when_shared_heavy_job_lock_is_held(
    tmp_path: Path, monkeypatch
) -> None:
    heavy_lock = tmp_path / "heavy.lock"
    monkeypatch.setattr(MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", heavy_lock)
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path)

    with MODULE.resource_guard._host_lock(heavy_lock, description="memory-heavy job"):
        assert MODULE.main(
            _guarded_args(tmp_path, executable)
        ) == MODULE.resource_guard.LOCK_UNAVAILABLE_EXIT_CODE


def test_runner_injects_private_staging_id_and_removes_failure_residue(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    body = """
import os
from pathlib import Path
import sys
arguments = sys.argv[1:]
staging_id = arguments[arguments.index("--staging-id") + 1]
out_dir = Path(arguments[arguments.index("--out-dir") + 1])
residue = out_dir.parent / f".kagemusha-v4-staging-{staging_id}-work"
(residue / "large-key.part").write_bytes(b"residue")
fd = int(os.environ["IROHA_RESOURCE_GUARD_AUTH_FD"])
token = os.environ["IROHA_RESOURCE_GUARD_AUTH_TOKEN"]
record = os.read(fd, 256)
expected = f"IROHA_RESOURCE_GUARD_AUTH_V1:{token}\\n".encode("ascii")
if record != expected:
    raise SystemExit(91)
raise SystemExit(9)
"""
    executable = _fake_prebuilt_generator(tmp_path, body)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 9
    assert not list(tmp_path.glob(".kagemusha-v4-staging-*-work"))
    assert not (tmp_path / "candidate").exists()
    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["post_run_cleanup"] == "completed"
    assert summary["post_run_cleanup_removed"] == 2
    assert summary["post_success_finalize"] == "skipped"


def test_runner_rejects_caller_supplied_staging_id(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    arguments = _guarded_args(tmp_path, executable)
    arguments.extend(["--staging-id", "0" * MODULE.STAGING_ID_HEX_LENGTH])

    assert MODULE.main(arguments) == 1
    assert not (tmp_path / "resource-report").exists()


def test_runner_rejects_symlinked_or_writable_executable(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    symlink_parent = tmp_path / "symlink-parent"
    symlink_parent.mkdir()
    symlink = symlink_parent / MODULE.BUNDLE_EXECUTABLE
    symlink.symlink_to(executable)

    assert MODULE.main(_guarded_args(tmp_path, symlink)) == 1
    assert not (tmp_path / "resource-report").exists()

    executable.chmod(0o722)
    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not (tmp_path / "resource-report").exists()


def test_runner_fails_if_executable_changes_during_run(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(
        tmp_path,
        """
from pathlib import Path
import sys
original = Path(sys.argv[0]).parents[1] / "kagemusha_recursive_spend_v4_bundle"
original.rename(original.with_name("admitted-original"))
original.write_text("#!/bin/sh\\nexit 0\\n", encoding="utf-8")
original.chmod(0o700)
(Path(sys.argv[0]).parents[1] / "admitted-copy-ran").write_text(
    "yes", encoding="ascii"
)
""",
    )

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["exit_reason"] == "post_run_validation_error"
    assert summary["post_run_validation"] == "failed"
    assert summary["post_success_finalize"] == "skipped"
    assert not (tmp_path / "candidate").exists()
    assert (tmp_path / "admitted-copy-ran").read_text(encoding="ascii") == "yes"


def test_stale_journal_recovers_only_its_exact_staging_directory(
    tmp_path: Path,
) -> None:
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        MODULE._create_run_journal(parent, staging_id)
        residue = tmp_path / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
        residue.mkdir(mode=0o700)
        (residue / "partial").write_bytes(b"partial")

        assert MODULE._recover_stale_runs(parent) == 1
        assert not residue.exists()
        assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    finally:
        parent.close()


def test_stale_journal_for_candidate_a_does_not_block_candidate_b(
    tmp_path: Path,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate-a"),
    ]
    second_command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate-b"),
    ]
    _guarded, first_parent, first_id = MODULE._prepare_guarded_command(first_command)
    try:
        MODULE._create_run_journal(first_parent, first_id)
        residue = tmp_path / f"{MODULE.STAGING_PREFIX}{first_id}-crash"
        residue.mkdir(mode=0o700)
    finally:
        first_parent.close()

    _guarded, second_parent, _second_id = MODULE._prepare_guarded_command(
        second_command
    )
    try:
        assert MODULE._recover_stale_runs(second_parent) == 1
        assert not residue.exists()
        assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    finally:
        second_parent.close()


def test_recovery_rejects_unjournaled_or_tampered_residue(tmp_path: Path) -> None:
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        unjournaled = tmp_path / f"{MODULE.STAGING_PREFIX}{staging_id}-unknown"
        unjournaled.mkdir(mode=0o700)
        with pytest.raises(MODULE.resource_guard.GuardError, match="unjournaled"):
            MODULE._recover_stale_runs(parent)
        unjournaled.rmdir()

        MODULE._create_run_journal(parent, staging_id)
        marker = tmp_path / MODULE._journal_name(staging_id)
        marker.write_text("{}\n", encoding="utf-8")
        marker.chmod(0o600)
        with pytest.raises(MODULE.resource_guard.GuardError, match="output leaf"):
            MODULE._recover_stale_runs(parent)
        assert marker.exists()
    finally:
        parent.close()


def test_journal_write_failure_removes_partial_marker(
    tmp_path: Path, monkeypatch
) -> None:
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)

    def fail_after_partial_write(descriptor: int, payload: bytes) -> None:
        os.write(descriptor, payload[: max(1, len(payload) // 2)])
        raise MODULE.resource_guard.GuardError("injected journal write failure")

    monkeypatch.setattr(MODULE.resource_guard, "_write_all", fail_after_partial_write)
    try:
        with pytest.raises(
            MODULE.resource_guard.GuardError, match="injected journal write failure"
        ):
            MODULE._create_run_journal(parent, staging_id)
        assert not (tmp_path / MODULE._journal_name(staging_id)).exists()
    finally:
        parent.close()


def test_uncertain_visible_publication_retains_recovery_journal(
    tmp_path: Path,
) -> None:
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        MODULE._create_run_journal(parent, staging_id)
        MODULE._create_staging_directory(parent, staging_id)
        (tmp_path / "candidate").mkdir(mode=0o700)

        with pytest.raises(MODULE.resource_guard.GuardError, match="retained"):
            MODULE._cleanup_guarded_run(parent, staging_id)
        assert (tmp_path / MODULE._journal_name(staging_id)).is_file()
    finally:
        (tmp_path / "candidate").rmdir()
        if (tmp_path / MODULE._journal_name(staging_id)).exists():
            MODULE._remove_run_journal(parent, staging_id)
        parent.close()


def test_crash_recovery_is_scoped_to_the_same_output_parent(tmp_path: Path) -> None:
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first_root.mkdir()
    second_root.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(first_root / "candidate"),
    ]
    second_command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(second_root / "candidate"),
    ]
    _guarded, first_parent, staging_id = MODULE._prepare_guarded_command(
        first_command
    )
    _guarded, second_parent, _second_id = MODULE._prepare_guarded_command(
        second_command
    )
    try:
        MODULE._create_run_journal(first_parent, staging_id)
        residue = first_root / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
        residue.mkdir(mode=0o700)

        assert MODULE._recover_stale_runs(second_parent) == 0
        assert residue.is_dir()
        assert MODULE._recover_stale_runs(first_parent) == 1
        assert not residue.exists()
    finally:
        first_parent.close()
        second_parent.close()


def test_pinned_output_parent_detects_path_replacement(tmp_path: Path) -> None:
    output_parent = tmp_path / "output-parent"
    output_parent.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)
    command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(output_parent / "candidate"),
    ]
    _guarded, parent, _staging_id = MODULE._prepare_guarded_command(command)
    moved = tmp_path / "moved-output-parent"
    try:
        output_parent.rename(moved)
        output_parent.mkdir()
        with pytest.raises(MODULE.resource_guard.GuardError, match="identity changed"):
            parent.validate()
        parent.validate(require_path=False)
    finally:
        parent.close()


def test_output_parent_requires_disk_backing_and_free_space(
    tmp_path: Path, monkeypatch
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "tmpfs")
    with pytest.raises(MODULE.resource_guard.GuardError, match="disk-backed"):
        MODULE._prepare_guarded_command(command)
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "ext4")
    actual_fstatvfs = os.fstatvfs

    class LowSpace:
        f_bavail = 1
        f_frsize = 4096

    monkeypatch.setattr(MODULE.os, "fstatvfs", lambda _descriptor: LowSpace())
    with pytest.raises(MODULE.resource_guard.GuardError, match="512 MiB"):
        MODULE._prepare_guarded_command(command)
    monkeypatch.setattr(MODULE.os, "fstatvfs", actual_fstatvfs)


def test_publisher_session_lifeline_prevents_orphaned_completion(
    tmp_path: Path,
) -> None:
    executable = tmp_path / MODULE.BUNDLE_EXECUTABLE
    marker = tmp_path / "publisher-completed"
    executable.write_text(
        f"#!{sys.executable}\n"
        "from pathlib import Path\n"
        "import sys\n"
        "import time\n"
        "time.sleep(10)\n"
        "Path(sys.argv[1]).write_text('published', encoding='ascii')\n",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    snapshot = MODULE._snapshot_executable(str(executable), MODULE.BUNDLE_EXECUTABLE)
    command = [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(tmp_path / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    MODULE._create_run_journal(parent, staging_id)
    MODULE._prepare_execution_copy(parent, snapshot, staging_id)
    session = MODULE._spawn_pinned_guarded_session(
        [snapshot.execution_path(), str(marker)],
        os.environ.copy(),
        (),
        (),
        snapshot,
    )
    try:
        os.close(session.lifeline_writer)
        session.lifeline_writer = -1
        assert session.wrapper.wait(timeout=6) != 0
        assert not marker.exists()
    finally:
        session.close()
        MODULE._release_execution_copy(parent, snapshot)
        MODULE._cleanup_guarded_run(parent, staging_id)
        parent.close()
        snapshot.close()
