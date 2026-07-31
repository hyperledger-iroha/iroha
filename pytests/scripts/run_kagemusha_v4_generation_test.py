"""Tests for the guarded Kagemusha V4 candidate-generation launcher."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT_DIR / "scripts" / "run_kagemusha_v4_generation.py"
SPEC = importlib.util.spec_from_file_location("run_kagemusha_v4_generation", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
ORIGINAL_REQUIRE_GENERATION_WORKER_IDENTITY = (
    MODULE._require_generation_worker_identity
)


@pytest.fixture(autouse=True)
def _admit_test_output_filesystem(monkeypatch) -> None:
    """Keep tmp_path portable while filesystem-policy tests override explicitly."""

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "ext4")
    monkeypatch.setattr(
        MODULE.published_build,
        "admit_candidate",
        lambda receipt, digest: SimpleNamespace(
            artifact_root=receipt.parent,
            artifact_tree_sha256="a" * 64,
            build_uid=501,
            build_user_name="boi-build",
            executable=receipt.resolve(strict=True),
            executable_sha256=digest,
            executable_size_bytes=receipt.stat().st_size,
            receipt=receipt.resolve(strict=True),
            receipt_sha256=digest,
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "_require_root_published_executable",
        lambda _snapshot, _admitted: None,
    )
    monkeypatch.setattr(
        MODULE,
        "_require_generation_worker_identity",
        lambda _admitted: None,
    )


def _worker_output_root(tmp_path: Path, name: str = "generation-worker-output") -> Path:
    return tmp_path / name


def _candidate_path(tmp_path: Path, name: str = "generation-worker-output") -> Path:
    return _worker_output_root(tmp_path, name) / "candidate"


def _report_path(tmp_path: Path, name: str = "generation-worker-output") -> Path:
    return _worker_output_root(tmp_path, name) / "resource-report"


def _generation_command(
    tmp_path: Path,
    executable: Path,
    *,
    root_name: str = "generation-worker-output",
    output_name: str = "candidate",
) -> list[str]:
    return [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(_worker_output_root(tmp_path, root_name) / output_name),
    ]


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
    executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
    return [
        "--resource-report",
        str(_report_path(tmp_path)),
        "--root-published-build-receipt",
        str(executable),
        "--root-published-build-receipt-sha256",
        executable_sha256,
        "--",
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(_candidate_path(tmp_path)),
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
    report_root = _report_path(tmp_path)

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
    assert _candidate_path(tmp_path).is_dir()
    assert summary["report_context"]["output_parent"]["canonical_path"] == str(
        _worker_output_root(tmp_path).resolve()
    )
    assert (
        summary["report_context"]["output_parent"]["admission"]
        == "fresh_single_use_generation_worker_output_parent"
    )
    assert (
        summary["report_context"]["publication_status"]
        == MODULE.GENERATION_PUBLICATION_STATUS
    )
    assert (
        summary["report_context"]["cross_stage_status"]
        == MODULE.GENERATION_CROSS_STAGE_STATUS
    )
    assert summary["report_context"]["root_published_build"]["build_uid"] == 501
    assert (
        summary["report_context"]["root_published_build"]["build_user_name"]
        == "boi-build"
    )
    expected_parent = summary["report_context"]["output_parent"]
    executable_identity = summary["report_context"]["executable_identity"]
    observations = (_candidate_path(tmp_path) / "fd-observations").read_text(
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
    assert not list(
        _worker_output_root(tmp_path).glob(f"{MODULE.JOURNAL_PREFIX}*")
    )
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


def test_runner_requires_receipt_named_executable(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    other_root = tmp_path / "other"
    other_root.mkdir()
    other = _fake_prebuilt_generator(other_root)
    arguments = _guarded_args(tmp_path, executable)
    receipt_position = arguments.index("--root-published-build-receipt") + 1
    digest_position = (
        arguments.index("--root-published-build-receipt-sha256") + 1
    )
    arguments[receipt_position] = str(other)
    arguments[digest_position] = hashlib.sha256(other.read_bytes()).hexdigest()

    assert MODULE.main(arguments) == 1
    assert not _report_path(tmp_path).exists()


def test_generation_requires_receipt_named_non_root_build_uid(
    monkeypatch,
) -> None:
    admitted = SimpleNamespace(
        build_uid=501,
        build_user_name=MODULE.GENERATION_WORKER_NAME,
    )
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 501)
    ORIGINAL_REQUIRE_GENERATION_WORKER_IDENTITY(admitted)

    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 502)
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="receipt-named non-root",
    ):
        ORIGINAL_REQUIRE_GENERATION_WORKER_IDENTITY(admitted)

    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 0)
    admitted.build_uid = 0
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="receipt-named non-root",
    ):
        ORIGINAL_REQUIRE_GENERATION_WORKER_IDENTITY(admitted)

    admitted.build_uid = 501
    admitted.build_user_name = "operator"
    monkeypatch.setattr(MODULE.os, "geteuid", lambda: 501)
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="receipt-named non-root",
    ):
        ORIGINAL_REQUIRE_GENERATION_WORKER_IDENTITY(admitted)


def test_production_finalization_rejects_direct_provisional_worker_path(
    tmp_path: Path,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    published_candidate = tmp_path / "root-published" / "candidate"
    published_candidate.mkdir(parents=True)
    provisional_candidate = tmp_path / "worker-output" / "candidate"
    provisional_candidate.mkdir(parents=True)
    admitted = SimpleNamespace(
        candidate_build=SimpleNamespace(executable=executable.resolve()),
        candidate_dir=published_candidate.resolve(),
    )
    command = [
        str(executable),
        "finalize-release",
        "--candidate-dir",
        str(provisional_candidate),
    ]

    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="rejects direct provisional worker paths",
    ):
        MODULE._validate_finalization_command(command, admitted)


def test_finalization_runner_admits_receipt_named_candidate(
    tmp_path: Path,
    monkeypatch,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    candidate = tmp_path / "root-published" / "candidate"
    candidate.mkdir(parents=True)
    admitted_build = SimpleNamespace(
        executable=executable.resolve(),
        executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
        executable_size_bytes=executable.stat().st_size,
    )
    receipt = tmp_path / MODULE.published_generated.RECEIPT_FILE_NAME
    receipt.write_bytes(b"generated receipt\n")
    launch_receipt = (
        tmp_path
        / MODULE.published_generated.WORKER_LAUNCH_RECEIPT_FILE_NAME
    )
    launch_receipt.write_bytes(b"launch receipt\n")
    admitted = SimpleNamespace(
        candidate_build=admitted_build,
        candidate_dir=candidate.resolve(),
        receipt=receipt.resolve(),
        receipt_sha256=hashlib.sha256(receipt.read_bytes()).hexdigest(),
        worker_launch_receipt=launch_receipt.resolve(),
        worker_launch_receipt_sha256=hashlib.sha256(
            launch_receipt.read_bytes()
        ).hexdigest(),
    )
    monkeypatch.setattr(
        MODULE.published_generated,
        "admit_generated_candidate",
        lambda _receipt, _digest: admitted,
    )
    invoked: list[list[str]] = []
    monkeypatch.setattr(
        MODULE,
        "_validate_finalization_loader_boundary",
        lambda _admitted: None,
    )
    monkeypatch.setattr(
        MODULE,
        "_open_finalization_receipt_descriptor",
        lambda path, _digest: os.open(path, os.O_RDONLY),
    )
    monkeypatch.setattr(
        MODULE,
        "_run_receipt_bound_finalization_command",
        lambda command, _snapshot, _descriptors: invoked.append(
            list(command)
        ),
    )
    arguments = [
        MODULE.GENERATED_CANDIDATE_RECEIPT_OPTION,
        str(receipt),
        MODULE.GENERATED_CANDIDATE_RECEIPT_SHA256_OPTION,
        "a" * 64,
        "--",
        str(executable),
        "finalize-release",
        "--candidate-dir",
        str(candidate),
    ]

    assert MODULE.main(arguments) == 0
    assert len(invoked) == 1
    assert invoked[0][0] == str(executable.resolve())
    assert invoked[0][1] == "finalize-release"
    assert MODULE.FINALIZATION_GENERATED_RECEIPT_FD_OPTION in invoked[0]
    assert MODULE.FINALIZATION_LAUNCH_RECEIPT_FD_OPTION in invoked[0]
    assert admitted.receipt_sha256 in invoked[0]
    assert admitted.worker_launch_receipt_sha256 in invoked[0]


def test_finalization_rejects_caller_supplied_receipt_capability(
    tmp_path: Path,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    candidate = tmp_path / "root-published" / "candidate"
    candidate.mkdir(parents=True)
    admitted = SimpleNamespace(
        candidate_build=SimpleNamespace(executable=executable.resolve()),
        candidate_dir=candidate.resolve(),
    )
    command = [
        str(executable),
        "finalize-release",
        "--candidate-dir",
        str(candidate),
        MODULE.FINALIZATION_LAUNCH_RECEIPT_FD_OPTION,
        "9",
    ]

    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="injected only after admission",
    ):
        MODULE._validate_finalization_command(command, admitted)


def test_finalization_environment_discards_loader_overrides(
    monkeypatch,
) -> None:
    monkeypatch.setenv("DYLD_INSERT_LIBRARIES", "/tmp/hostile.dylib")
    monkeypatch.setenv("LD_PRELOAD", "/tmp/hostile.so")
    monkeypatch.setenv("UNRELATED_SECRET", "must-not-propagate")

    environment = MODULE._sanitized_finalization_environment()

    assert environment == {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": "/private/tmp",
        "TZ": "UTC",
    }


def test_finalization_loader_boundary_is_deny_by_default_off_macos(
    monkeypatch,
) -> None:
    monkeypatch.setattr(MODULE.sys, "platform", "linux")
    admitted = SimpleNamespace(
        artifact_root=Path("/root/published"),
        executable=Path("/root/published/kagemusha_recursive_spend_v4_bundle"),
    )

    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="requires the reviewed macOS",
    ):
        MODULE._validate_finalization_loader_boundary(admitted)


def test_finalization_loader_boundary_scans_the_full_admitted_closure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    inspector = (tmp_path / "otool").resolve()
    inspector.write_bytes(b"immutable inspector\n")
    inspector.chmod(0o500)
    executable = _fake_prebuilt_generator(tmp_path)
    admitted = SimpleNamespace(
        artifact_root=tmp_path.resolve(),
        executable=executable.resolve(),
    )
    observed: list[tuple[Path, tuple[Path, ...], Path]] = []
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    monkeypatch.setattr(
        MODULE.published_build,
        "TRUSTED_OWNER_UID",
        os.geteuid(),
    )
    monkeypatch.setattr(
        MODULE.candidate_builder,
        "_admit_macos_dynamic_tool_closure",
        lambda root, executables, *, otool: observed.append(
            (root, tuple(executables), otool)
        ),
    )

    MODULE._validate_finalization_loader_boundary(
        admitted,
        otool=inspector,
    )

    assert observed == [
        (tmp_path.resolve(), (executable.resolve(),), inspector)
    ]


def test_generic_runner_is_explicitly_diagnostic_only(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    with pytest.raises(SystemExit):
        MODULE.parse_args(
            [
                "--report",
                str(tmp_path / "report.json"),
                "--",
                "/usr/bin/true",
            ]
        )
    with pytest.raises(ValueError, match="diagnostic-only"):
        MODULE._reject_production_command_in_generic_runner(
            [
                str(executable),
                "finalize-release",
                "--candidate-dir",
                str(tmp_path / "candidate"),
            ]
        )


def test_runner_refuses_preexisting_reusable_output_parent(tmp_path: Path) -> None:
    worker_root = _worker_output_root(tmp_path)
    worker_root.mkdir()
    (worker_root / "resource-report").mkdir()
    executable = _fake_prebuilt_generator(tmp_path)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not _candidate_path(tmp_path).exists()


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
    assert not list(
        _worker_output_root(tmp_path).glob(".kagemusha-v4-staging-*-work")
    )
    assert not _candidate_path(tmp_path).exists()
    summary = json.loads(
        (_report_path(tmp_path) / "kagemusha_resource_summary.json").read_text(
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
    assert not _report_path(tmp_path).exists()


def test_runner_rejects_symlinked_or_writable_executable(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    symlink_parent = tmp_path / "symlink-parent"
    symlink_parent.mkdir()
    symlink = symlink_parent / MODULE.BUNDLE_EXECUTABLE
    symlink.symlink_to(executable)

    assert MODULE.main(_guarded_args(tmp_path, symlink)) == 1
    assert not _report_path(tmp_path).exists()

    executable.chmod(0o722)
    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not _report_path(tmp_path).exists()


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
original = Path(sys.argv[0]).parents[2] / "kagemusha_recursive_spend_v4_bundle"
original.rename(original.with_name("admitted-original"))
original.write_text("#!/bin/sh\\nexit 0\\n", encoding="utf-8")
original.chmod(0o700)
(Path(sys.argv[0]).parents[2] / "admitted-copy-ran").write_text(
    "yes", encoding="ascii"
)
""",
    )

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    summary = json.loads(
        (_report_path(tmp_path) / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["exit_reason"] == "post_run_validation_error"
    assert summary["post_run_validation"] == "failed"
    assert summary["post_success_finalize"] == "skipped"
    assert not _candidate_path(tmp_path).exists()
    assert (tmp_path / "admitted-copy-ran").read_text(encoding="ascii") == "yes"


def test_stale_journal_recovers_only_its_exact_staging_directory(
    tmp_path: Path,
) -> None:
    command = _generation_command(
        tmp_path,
        _fake_prebuilt_generator(tmp_path),
    )
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        MODULE._create_run_journal(parent, staging_id)
        residue = parent.path / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
        residue.mkdir(mode=0o700)
        (residue / "partial").write_bytes(b"partial")

        assert MODULE._recover_stale_runs(parent) == 1
        assert not residue.exists()
        assert not list(parent.path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    finally:
        parent.close()


def test_fresh_output_parent_cannot_be_reused_after_a_prior_run(
    tmp_path: Path,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = _generation_command(tmp_path, executable, output_name="candidate-a")
    _guarded, first_parent, _first_id = MODULE._prepare_guarded_command(
        first_command
    )
    first_parent.close()

    second_command = _generation_command(
        tmp_path,
        executable,
        output_name="candidate-b",
    )
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="preexisting or reusable",
    ):
        MODULE._prepare_guarded_command(second_command)


def test_fresh_output_parent_contains_the_resource_report(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    command = _generation_command(tmp_path, executable)
    _guarded, parent, _staging_id = MODULE._prepare_guarded_command(command)
    try:
        with pytest.raises(
            MODULE.resource_guard.GuardError,
            match="inside the fresh single-use",
        ):
            MODULE._prepare_report_directory(
                tmp_path / "outside-report",
                output_parent=parent,
            )
        report, summary = MODULE._prepare_report_directory(
            parent.path / "resource-report",
            output_parent=parent,
        )
        assert report.parent == parent.path / "resource-report"
        assert summary.parent == report.parent
    finally:
        parent.close()


def test_recovery_rejects_unjournaled_or_tampered_residue(tmp_path: Path) -> None:
    command = _generation_command(
        tmp_path,
        _fake_prebuilt_generator(tmp_path),
    )
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        unjournaled = parent.path / f"{MODULE.STAGING_PREFIX}{staging_id}-unknown"
        unjournaled.mkdir(mode=0o700)
        with pytest.raises(MODULE.resource_guard.GuardError, match="unjournaled"):
            MODULE._recover_stale_runs(parent)
        unjournaled.rmdir()

        MODULE._create_run_journal(parent, staging_id)
        marker = parent.path / MODULE._journal_name(staging_id)
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
    command = _generation_command(
        tmp_path,
        _fake_prebuilt_generator(tmp_path),
    )
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
        assert not (parent.path / MODULE._journal_name(staging_id)).exists()
    finally:
        parent.close()


def test_uncertain_visible_publication_retains_recovery_journal(
    tmp_path: Path,
) -> None:
    command = _generation_command(
        tmp_path,
        _fake_prebuilt_generator(tmp_path),
    )
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    try:
        MODULE._create_run_journal(parent, staging_id)
        MODULE._create_staging_directory(parent, staging_id)
        (parent.path / "candidate").mkdir(mode=0o700)

        with pytest.raises(MODULE.resource_guard.GuardError, match="retained"):
            MODULE._cleanup_guarded_run(parent, staging_id)
        assert (parent.path / MODULE._journal_name(staging_id)).is_file()
    finally:
        (parent.path / "candidate").rmdir()
        if (parent.path / MODULE._journal_name(staging_id)).exists():
            MODULE._remove_run_journal(parent, staging_id)
        parent.close()


def test_crash_recovery_is_scoped_to_the_same_output_parent(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = _generation_command(
        tmp_path,
        executable,
        root_name="first-worker-output",
    )
    second_command = _generation_command(
        tmp_path,
        executable,
        root_name="second-worker-output",
    )
    _guarded, first_parent, staging_id = MODULE._prepare_guarded_command(
        first_command
    )
    _guarded, second_parent, _second_id = MODULE._prepare_guarded_command(
        second_command
    )
    try:
        MODULE._create_run_journal(first_parent, staging_id)
        residue = first_parent.path / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
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
    executable = _fake_prebuilt_generator(tmp_path)
    command = _generation_command(
        tmp_path,
        executable,
        root_name=output_parent.name,
    )
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
    command = _generation_command(tmp_path, executable)

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "tmpfs")
    with pytest.raises(MODULE.resource_guard.GuardError, match="disk-backed"):
        MODULE._prepare_guarded_command(command)
    rejected_parent = _worker_output_root(tmp_path)
    assert not list(rejected_parent.glob(f"{MODULE.STAGING_PREFIX}*"))
    assert not list(rejected_parent.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    rejected_parent.rmdir()

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
    command = _generation_command(tmp_path, executable)
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


def test_pinned_session_exposes_only_the_exact_executable_descriptor(
    tmp_path: Path,
) -> None:
    executable = tmp_path / MODULE.BUNDLE_EXECUTABLE
    marker = tmp_path / "descriptor-authenticated"
    executable.write_text(
        f"#!{sys.executable}\n"
        "import os\n"
        "from pathlib import Path\n"
        "import sys\n"
        f"fd_text = os.environ.get({MODULE.EXECUTABLE_FD_ENV!r}, '')\n"
        "if not fd_text.isdecimal() or int(fd_text) < 3:\n"
        "    raise SystemExit(81)\n"
        "fd = int(fd_text)\n"
        "opened = os.fstat(fd)\n"
        "invoked = os.stat(sys.argv[0], follow_symlinks=True)\n"
        "if (opened.st_dev, opened.st_ino) != (invoked.st_dev, invoked.st_ino):\n"
        "    raise SystemExit(82)\n"
        "os.lseek(fd, 0, os.SEEK_SET)\n"
        "if b'descriptor-authenticated' not in os.read(fd, opened.st_size):\n"
        "    raise SystemExit(83)\n"
        "Path(sys.argv[1]).write_text('descriptor-authenticated', encoding='ascii')\n",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    snapshot = MODULE._snapshot_executable(str(executable), MODULE.BUNDLE_EXECUTABLE)
    command = _generation_command(tmp_path, executable)
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    MODULE._create_run_journal(parent, staging_id)
    MODULE._prepare_execution_copy(parent, snapshot, staging_id)
    environment = os.environ.copy()
    environment[MODULE.EXECUTABLE_FD_ENV] = "999999"
    session = MODULE._spawn_pinned_guarded_session(
        [snapshot.execution_path(), str(marker)],
        environment,
        (),
        (),
        snapshot,
    )
    try:
        assert session.wrapper.wait(timeout=6) == 0
        assert (
            session.control.read_line(
                timeout=2,
                description="exact executable descriptor test",
            ).split()[:3]
            == ["EXIT", "0", "0"]
        )
        assert marker.read_text(encoding="ascii") == "descriptor-authenticated"
    finally:
        session.close()
        MODULE._release_execution_copy(parent, snapshot)
        MODULE._cleanup_guarded_run(parent, staging_id)
        parent.close()
        snapshot.close()


def test_darwin_root_owned_executable_still_uses_private_execution_copy(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.setattr(MODULE.sys, "platform", "darwin")
    executable = _fake_prebuilt_generator(tmp_path)
    snapshot = MODULE._snapshot_executable(
        str(executable),
        MODULE.BUNDLE_EXECUTABLE,
    )
    snapshot.owner_uid = MODULE.published_build.TRUSTED_OWNER_UID
    command = _generation_command(tmp_path, executable)
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    MODULE._create_run_journal(parent, staging_id)
    try:
        MODULE._prepare_execution_copy(parent, snapshot, staging_id)
        assert snapshot.execution_copy is not None
        assert snapshot.execution_path() == str(snapshot.execution_copy.path)
        assert (
            snapshot.report_context()["execution"]["method"]
            == "darwin_private_fd_copy"
        )
    finally:
        MODULE._release_execution_copy(parent, snapshot)
        MODULE._cleanup_guarded_run(parent, staging_id)
        parent.close()
        snapshot.close()
