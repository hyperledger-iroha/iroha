"""Tests for the SoraFS direct-mode smoke wrapper."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "sorafs_direct_mode_smoke.sh"
MANIFEST_ID = "a" * 64
PROVIDER_SPEC = (
    "name=provider-a,"
    f"provider-id={'b' * 64},"
    "base-url=https://provider-a.example,"
    "stream-token=dGVzdA=="
)


def write_inputs(tmp_path: Path, policy: dict | None = None) -> tuple[Path, Path]:
    """Create minimal existing plan and policy files for wrapper preflight."""

    plan_path = tmp_path / "plan.json"
    policy_path = tmp_path / "policy.json"
    plan_path.write_text("{}", encoding="utf-8")
    policy_path.write_text(json.dumps(policy or {}), encoding="utf-8")
    return plan_path, policy_path


def run_wrapper(
    tmp_path: Path,
    plan_path: Path,
    policy_path: Path,
    *extra_args: str,
) -> subprocess.CompletedProcess[str]:
    """Run the direct-mode smoke wrapper with a CLI that must not be reached."""

    return subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(tmp_path),
            "--plan",
            str(plan_path),
            "--manifest-id",
            MANIFEST_ID,
            "--policy",
            str(policy_path),
            "--provider",
            PROVIDER_SPEC,
            "--cli",
            "/bin/false",
            *extra_args,
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def test_direct_mode_smoke_rejects_symlinked_payload_output(
    tmp_path: Path,
) -> None:
    plan_path, policy_path = write_inputs(tmp_path)
    target = tmp_path / "payload-target.bin"
    output = tmp_path / "payload.bin"
    target.write_bytes(b"existing")
    output.symlink_to(target)

    result = run_wrapper(
        tmp_path,
        plan_path,
        policy_path,
        "--skip-adoption-check",
        "--output",
        str(output),
        "--summary",
        str(tmp_path / "summary.json"),
    )

    assert result.returncode == 1
    assert "payload output must not be a symlink" in result.stderr
    assert not (tmp_path / "summary.json").exists()


def test_direct_mode_smoke_rejects_symlinked_payload_parent(
    tmp_path: Path,
) -> None:
    plan_path, policy_path = write_inputs(tmp_path)
    real_dir = tmp_path / "real-output"
    linked_dir = tmp_path / "linked-output"
    real_dir.mkdir()
    linked_dir.symlink_to(real_dir, target_is_directory=True)

    result = run_wrapper(
        tmp_path,
        plan_path,
        policy_path,
        "--skip-adoption-check",
        "--output",
        str(linked_dir / "payload.bin"),
        "--summary",
        str(tmp_path / "summary.json"),
    )

    assert result.returncode == 1
    assert "payload output parent must not be a symlink" in result.stderr
    assert not (real_dir / "payload.bin").exists()


def test_direct_mode_smoke_rejects_symlinked_summary_output(
    tmp_path: Path,
) -> None:
    plan_path, policy_path = write_inputs(tmp_path)
    summary_target = tmp_path / "summary-target.json"
    summary = tmp_path / "summary.json"
    summary_target.write_text("{}", encoding="utf-8")
    summary.symlink_to(summary_target)

    result = run_wrapper(
        tmp_path,
        plan_path,
        policy_path,
        "--skip-adoption-check",
        "--output",
        str(tmp_path / "payload.bin"),
        "--summary",
        str(summary),
    )

    assert result.returncode == 1
    assert "summary JSON report must not be a symlink" in result.stderr
    assert not (tmp_path / "payload.bin").exists()


def test_direct_mode_smoke_rejects_symlinked_adoption_report(
    tmp_path: Path,
) -> None:
    plan_path, policy_path = write_inputs(tmp_path)
    report_target = tmp_path / "adoption-target.json"
    report = tmp_path / "adoption.json"
    report_target.write_text("{}", encoding="utf-8")
    report.symlink_to(report_target)

    result = run_wrapper(
        tmp_path,
        plan_path,
        policy_path,
        "--output",
        str(tmp_path / "payload.bin"),
        "--summary",
        str(tmp_path / "summary.json"),
        "--adoption-report",
        str(report),
    )

    assert result.returncode == 1
    assert "adoption report must not be a symlink" in result.stderr
    assert not (tmp_path / "payload.bin").exists()


def test_direct_mode_smoke_rejects_policy_scoreboard_symlink(
    tmp_path: Path,
) -> None:
    scoreboard_target = tmp_path / "scoreboard-target.json"
    scoreboard = tmp_path / "scoreboard.json"
    scoreboard_target.write_text("{}", encoding="utf-8")
    scoreboard.symlink_to(scoreboard_target)
    plan_path, policy_path = write_inputs(
        tmp_path,
        {"scoreboard": {"persist_path": str(scoreboard)}},
    )

    result = run_wrapper(
        tmp_path,
        plan_path,
        policy_path,
        "--skip-adoption-check",
        "--output",
        str(tmp_path / "payload.bin"),
        "--summary",
        str(tmp_path / "summary.json"),
    )

    assert result.returncode == 1
    assert "scoreboard output must not be a symlink" in result.stderr
    assert not (tmp_path / "payload.bin").exists()
