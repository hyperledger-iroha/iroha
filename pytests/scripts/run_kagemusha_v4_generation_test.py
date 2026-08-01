"""Tests for the guarded Kagemusha V4 candidate-generation launcher."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shlex
import stat
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


def test_runner_disables_local_bytecode_before_importing_guard_modules() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")
    disable_offset = source.index("sys.dont_write_bytecode = True")
    local_import_offset = source.index(
        "from scripts.formal import run_sumeragi_v2_tlapm_guard"
    )

    assert disable_offset < local_import_offset


def _fake_prebuilt_generator(tmp_path: Path, mode: str = "success") -> Path:
    """Create a loader-stable shell double for the admitted native generator.

    These integration-style tests exercise the Python supervisor itself. A
    second Homebrew Python process is not part of that contract and can spend
    minutes blocked in dyld on a loaded macOS builder, so the child double uses
    the OS shell and shell builtins instead.
    """

    if mode not in {"success", "fail_with_residue", "replace_original"}:
        raise ValueError(f"unsupported fake generator mode: {mode}")
    executable = tmp_path / "kagemusha_recursive_spend_v4_bundle"
    helper = tmp_path / f"fake-kagemusha-generator-{mode}.sh"
    helper.write_text(
        f"""#!/bin/sh
set -eu
mode={mode!r}
execution_path=$1
shift
operation=$1
shift
out_dir=
staging_id=
staging_name=
parent_fd=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --out-dir) out_dir=$2; shift 2 ;;
        --staging-id) staging_id=$2; shift 2 ;;
        --staging-name) staging_name=$2; shift 2 ;;
        --output-parent-fd) parent_fd=$2; shift 2 ;;
        *) shift ;;
    esac
done
case "$parent_fd" in
    ''|*[!0-9]*) exit 92 ;;
esac
[ "$staging_name" = ".kagemusha-v4-staging-${{staging_id}}-work" ] || exit 93
parent_path=/dev/fd/$parent_fd
[ -d "$parent_path" ] || exit 94
output_parent=${{out_dir%/*}}
staging_path="$output_parent/$staging_name"
[ -d "$staging_path" ] || exit 94
auth_fd=${{IROHA_RESOURCE_GUARD_AUTH_FD-}}
case "$auth_fd" in
    ''|*[!0-9]*) exit 91 ;;
esac
eval "IFS= read -r auth_record <&$auth_fd"
expected_auth="IROHA_RESOURCE_GUARD_AUTH_V1:${{IROHA_RESOURCE_GUARD_AUTH_TOKEN-}}"
[ "$auth_record" = "$expected_auth" ] || exit 91
parent_observation=$output_parent
printf '%s\t%s\t%s\n' "$operation" "$parent_observation" "$execution_path" \
    >> "$staging_path/fd-observations"
if [ "$mode" = fail_with_residue ] && [ "$operation" = generate-candidate ]; then
    printf residue > "$staging_path/large-key.part"
    exit 9
fi
if [ "$mode" = replace_original ] && [ "$operation" = generate-candidate ]; then
    execution_dir=${{execution_path%/*}}
    original_parent=${{execution_dir%/*}}
    original="$original_parent/kagemusha_recursive_spend_v4_bundle"
    /bin/mv "$original" "$original_parent/admitted-original"
    printf '#!/bin/sh\nexit 0\n' > "$original"
    chmod 700 "$original"
    printf yes > "$original_parent/admitted-copy-ran"
fi
if [ "$operation" = publish-staged-candidate ]; then
    /bin/mv "$staging_path" "$out_dir"
fi
""",
        encoding="utf-8",
    )
    helper.chmod(0o600)
    executable.write_text(
        "#!/bin/sh\n"
        f"exec /bin/sh {shlex.quote(str(helper))} \"$0\" \"$@\"\n",
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

    assert MODULE._effective_memory_limit_bytes(None) == 6 * MODULE.BYTES_PER_GIB
    assert MODULE._effective_memory_limit_bytes(0.125) == 128 * 1024 * 1024
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(7)
    with pytest.raises(MODULE.resource_guard.GuardError, match="greater than zero"):
        MODULE._effective_memory_limit_bytes(float("nan"))

    monkeypatch.setattr(
        MODULE,
        "_physical_memory_bytes",
        lambda: 64 * MODULE.BYTES_PER_GIB,
    )
    assert (
        MODULE._effective_memory_limit_bytes(None)
        == 32 * MODULE.BYTES_PER_GIB
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(32.01)

    monkeypatch.setattr(
        MODULE,
        "_physical_memory_bytes",
        lambda: 128 * MODULE.BYTES_PER_GIB,
    )
    assert (
        MODULE._effective_memory_limit_bytes(None)
        == 64 * MODULE.BYTES_PER_GIB
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(64.01)

    monkeypatch.setattr(MODULE, "_physical_memory_bytes", lambda: 0)
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="could not determine installed physical memory",
    ):
        MODULE._effective_memory_limit_bytes(None)


def test_physical_memory_detection_fails_closed(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.sys, "platform", "unsupported-posix")
    monkeypatch.setattr(MODULE.os, "sysconf", lambda _name: None)

    assert MODULE._physical_memory_bytes() == 0


def test_runner_executes_small_owned_group_and_writes_reports(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    report_root = tmp_path / "resource-report"

    executable = _fake_prebuilt_generator(tmp_path)
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
    executable_identity = summary["report_context"]["executable_identity"]
    observations = (tmp_path / "candidate" / "fd-observations").read_text(
        encoding="ascii"
    ).splitlines()
    assert len(observations) == 2
    execution_paths: list[str] = []
    for operation, observation in zip(
        ("generate-candidate", "publish-staged-candidate"), observations
    ):
        fields = observation.split("\t")
        assert fields[:2] == [
            operation,
            summary["report_context"]["output_parent"]["canonical_path"],
        ]
        assert len(fields) == 3
        execution_paths.append(fields[2])
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
    assert (
        summary["memory_enforcement_mode"]
        == MODULE.resource_guard.MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT
    )
    assert summary["sample_interval_seconds"] == MODULE.SAMPLE_INTERVAL_SECONDS
    assert (
        summary["physical_footprint_interval_seconds"]
        == MODULE.SAMPLE_INTERVAL_SECONDS
    )
    assert (report_root / "kagemusha_resource.jsonl").stat().st_size > 0


def test_runner_does_not_use_the_retired_boolean_supervision_marker() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")

    assert "IROHA_KAGEMUSHA_V4_RESOURCE_SUPERVISED" not in source
    assert "held_lock_descriptors=(heavy_lock, kagemusha_lock)" in source
    assert MODULE.ABSOLUTE_MAX_MEMORY_BYTES == 64 * MODULE.BYTES_PER_GIB
    assert MODULE.SAMPLE_INTERVAL_SECONDS == 0.25
    assert "sample_interval_seconds=SAMPLE_INTERVAL_SECONDS" in source
    assert "physical_footprint_interval_seconds" in source
    assert "MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT" in source


def test_runner_refuses_retired_rss_only_report_mode(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    report = tmp_path / "legacy-report.json"

    assert MODULE.main(["--report", str(report), "--", "/usr/bin/true"]) == 2

    captured = capsys.readouterr()
    assert "--report mode is retired" in captured.err
    assert "RSS-only" in captured.err
    assert "--resource-report" in captured.err
    assert not report.exists()
    source = RUNNER_PATH.read_text(encoding="utf-8")
    assert "run_guarded_command" not in source


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
    executable = _fake_prebuilt_generator(tmp_path, "fail_with_residue")

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


def test_cleanup_staging_is_fd_relative_and_does_not_follow_symlinks(
    tmp_path: Path,
) -> None:
    outside = tmp_path / "outside"
    outside.mkdir()
    sentinel = outside / "retain"
    sentinel.write_bytes(b"outside")
    output_parent = tmp_path / "output"
    output_parent.mkdir()
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(output_parent / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    residue_name = f"{MODULE.STAGING_PREFIX}{staging_id}-work"
    residue = output_parent / residue_name
    try:
        residue.mkdir(mode=0o700)
        nested = residue / "nested"
        nested.mkdir(mode=0o700)
        (nested / "partial").write_bytes(b"partial")
        (residue / "outside-link").symlink_to(outside, target_is_directory=True)
        nested.chmod(0o500)
        residue.chmod(0o500)
        moved_parent = tmp_path / "moved-output"
        output_parent.rename(moved_parent)
        output_parent.mkdir()
        decoy = output_parent / residue_name
        decoy.mkdir(mode=0o700)

        assert MODULE._cleanup_staging(parent, staging_id) == 1
        assert not (moved_parent / residue_name).exists()
        assert decoy.is_dir()
        assert sentinel.read_bytes() == b"outside"
    finally:
        parent.close()


def test_cleanup_never_path_chmods_a_swappable_entry(
    tmp_path: Path, monkeypatch
) -> None:
    outside = tmp_path / "outside"
    outside.mkdir(mode=0o755)
    outside.chmod(0o755)
    output_parent = tmp_path / "output"
    output_parent.mkdir()
    command = [
        str(_fake_prebuilt_generator(tmp_path)),
        "generate-candidate",
        "--out-dir",
        str(output_parent / "candidate"),
    ]
    _guarded, parent, staging_id = MODULE._prepare_guarded_command(command)
    residue_name = f"{MODULE.STAGING_PREFIX}{staging_id}-work"
    residue = output_parent / residue_name
    residue.mkdir(mode=0o700)
    (residue / "partial").write_bytes(b"partial")
    displaced = output_parent / "attacker-displaced-residue"
    real_chmod = os.chmod
    path_chmod_called = False

    def swap_entry_before_chmod(
        path: str, mode: int, *, dir_fd: int | None = None
    ) -> None:
        nonlocal path_chmod_called
        path_chmod_called = True
        residue.rename(displaced)
        residue.symlink_to(outside, target_is_directory=True)
        real_chmod(path, mode, dir_fd=dir_fd)

    monkeypatch.setattr(MODULE.os, "chmod", swap_entry_before_chmod)
    try:
        assert MODULE._cleanup_staging(parent, staging_id) == 1
        assert not path_chmod_called
        assert stat.S_IMODE(outside.stat().st_mode) == 0o755
    finally:
        parent.close()


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
    executable = _fake_prebuilt_generator(tmp_path, "replace_original")

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
    with pytest.raises(MODULE.resource_guard.GuardError, match="16 GiB"):
        MODULE._prepare_guarded_command(command)
    monkeypatch.setattr(MODULE.os, "fstatvfs", actual_fstatvfs)


def test_publisher_session_lifeline_prevents_orphaned_completion(
    tmp_path: Path,
) -> None:
    executable = tmp_path / MODULE.BUNDLE_EXECUTABLE
    marker = tmp_path / "publisher-completed"
    executable.write_text(
        "#!/bin/sh\n"
        "/bin/sleep 10\n"
        "printf published > \"$1\"\n",
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
