"""Tests for the isolated Kagemusha compact-generation memory benchmark."""

from __future__ import annotations

import importlib.util
import hashlib
import json
from pathlib import Path
import sys


ROOT_DIR = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT_DIR / "scripts" / "run_kagemusha_v4_generation_benchmark.py"
BENCHMARK_SOURCE_PATH = (
    ROOT_DIR
    / "crates"
    / "iroha_core"
    / "src"
    / "bin"
    / "kagemusha_recursive_spend_v4_memory_benchmark.rs"
)
CANDIDATE_RUNNER_PATH = ROOT_DIR / "scripts" / "run_kagemusha_v4_generation.py"
CORE_MANIFEST_PATH = ROOT_DIR / "crates" / "iroha_core" / "Cargo.toml"
SPEC = importlib.util.spec_from_file_location(
    "run_kagemusha_v4_generation_benchmark", RUNNER_PATH
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _fake_benchmark(tmp_path: Path, body: str = "") -> Path:
    executable = tmp_path / MODULE.BENCHMARK_EXECUTABLE
    executable.write_text(f"#!{sys.executable}\n" + body, encoding="utf-8")
    executable.chmod(0o700)
    return executable


def _guarded_args(tmp_path: Path, executable: Path) -> list[str]:
    scratch_parent = tmp_path / "scratch-parent"
    scratch_parent.mkdir(mode=0o700, exist_ok=True)
    scratch_parent.chmod(0o700)
    return [
        "--resource-report",
        str(tmp_path / "resource-report"),
        "--scratch-parent",
        str(scratch_parent),
        "--",
        str(executable),
        MODULE.BENCHMARK_SUBCOMMAND,
    ]


def test_runner_rejects_every_command_except_the_exact_benchmark(
    tmp_path: Path,
) -> None:
    report = tmp_path / "resource-report"
    scratch_parent = tmp_path / "scratch-parent"
    scratch_parent.mkdir(mode=0o700)
    fake_benchmark = _fake_benchmark(tmp_path)
    candidate_bundle = tmp_path / "kagemusha_recursive_spend_v4_bundle"
    candidate_bundle.write_text(f"#!{sys.executable}\n", encoding="utf-8")
    candidate_bundle.chmod(0o700)
    rejected_commands = [
        ["cargo", "run"],
        [str(candidate_bundle), "generate-candidate"],
        [str(fake_benchmark), "wrong-operation"],
        [str(fake_benchmark), MODULE.BENCHMARK_SUBCOMMAND, "extra"],
    ]

    for command in rejected_commands:
        assert (
            MODULE.main(
                [
                    "--resource-report",
                    str(report),
                    "--scratch-parent",
                    str(scratch_parent),
                    "--",
                    *command,
                ]
            )
            == 1
        )
        assert not report.exists()


def test_runner_executes_an_owned_group_and_writes_reports(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(
        MODULE.candidate_guard, "LOCK_PATH", tmp_path / "kagemusha.lock"
    )
    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "apfs")
    consume_capability = """
import os
from pathlib import Path
import tempfile
fd = int(os.environ["IROHA_RESOURCE_GUARD_AUTH_FD"])
token = os.environ["IROHA_RESOURCE_GUARD_AUTH_TOKEN"]
record = os.read(fd, 256)
expected = f"IROHA_RESOURCE_GUARD_AUTH_V1:{token}\\n".encode("ascii")
if record != expected:
    raise SystemExit(91)
scratch = os.environ["TMPDIR"]
if not Path(scratch).name.startswith(".kagemusha-v4-benchmark-scratch-"):
    raise SystemExit(92)
with tempfile.TemporaryFile(dir=scratch) as payload:
    payload.write(b"disk-backed scratch")
"""
    executable = _fake_benchmark(tmp_path, consume_capability)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 0
    report_root = tmp_path / "resource-report"
    summary = json.loads(
        (report_root / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["exit_reason"] == "completed"
    assert summary["exit_status"] == 0
    assert (
        0
        < summary["memory_limit_bytes"]
        <= MODULE.candidate_guard.ABSOLUTE_MAX_MEMORY_BYTES
    )
    assert (
        summary["sample_interval_seconds"]
        == MODULE.candidate_guard.SAMPLE_INTERVAL_SECONDS
    )
    assert summary["post_run_cleanup"] == "completed"
    assert summary["post_run_validation"] == "completed"
    executable_identity = summary["report_context"]["executable_identity"]
    assert executable_identity["sha256"] == hashlib.sha256(
        executable.read_bytes()
    ).hexdigest()
    scratch = summary["report_context"]["scratch"]
    assert scratch["filesystem_type"] == "apfs"
    assert scratch["ambient_temp_environment_ignored"] is True
    assert scratch["run_device"] > 0
    assert scratch["run_inode"] > 0
    assert not Path(scratch["canonical_run_directory"]).exists()
    assert (report_root / "kagemusha_resource.jsonl").stat().st_size > 0


def test_runner_rejects_memory_backed_scratch_parent(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(
        MODULE.candidate_guard, "LOCK_PATH", tmp_path / "kagemusha.lock"
    )
    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "tmpfs")
    executable = _fake_benchmark(tmp_path)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not list((tmp_path / "scratch-parent").iterdir())


def test_cleanup_rejects_a_replaced_scratch_entry(
    tmp_path: Path, monkeypatch
) -> None:
    parent = tmp_path / "scratch-parent"
    parent.mkdir(mode=0o700)
    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "apfs")
    scratch = MODULE._prepare_scratch_directory(parent)
    moved = parent / "moved-scratch"
    scratch.path.rename(moved)
    scratch.path.mkdir(mode=0o700)

    try:
        try:
            MODULE._cleanup_scratch_directory(scratch)
        except MODULE.resource_guard.GuardError as error:
            assert "identity changed" in str(error)
        else:
            raise AssertionError("replaced scratch entry was accepted")
    finally:
        scratch.path.rmdir()
        moved.rmdir()


def test_benchmark_source_cannot_frame_or_publish_a_candidate() -> None:
    benchmark_source = BENCHMARK_SOURCE_PATH.read_text(encoding="utf-8")
    candidate_runner_source = CANDIDATE_RUNNER_PATH.read_text(encoding="utf-8")
    core_manifest = CORE_MANIFEST_PATH.read_text(encoding="utf-8")

    assert "claim_kagemusha_generation_supervisor_permit_v4" in benchmark_source
    assert "generate_kagemusha_pasta_cycle_artifacts_v4" in benchmark_source
    assert benchmark_source.count("tempfile::tempfile()") == 2
    assert "KagemushaRecursiveSpendCandidateV4" not in benchmark_source
    assert "generate-candidate" not in benchmark_source
    assert "write_kagemusha_pasta_cycle_artifact" not in benchmark_source
    assert "--out-dir" not in benchmark_source
    assert f'name = "{MODULE.BENCHMARK_EXECUTABLE}"' in core_manifest
    assert (
        'required-features = ["zk-halo2-ipa", '
        '"kagemusha-generation-memory-lab"]'
        in core_manifest
    )
    assert MODULE.BENCHMARK_EXECUTABLE not in candidate_runner_source
    assert MODULE.BENCHMARK_SUBCOMMAND not in candidate_runner_source
