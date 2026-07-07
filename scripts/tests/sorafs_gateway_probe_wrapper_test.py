"""Tests for the SoraFS gateway probe telemetry wrapper."""

from __future__ import annotations

import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
PROBE_WRAPPER = REPO_ROOT / "scripts" / "telemetry" / "run_sorafs_gateway_probe.sh"


def run_wrapper(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the gateway probe wrapper and return its completed process."""
    return subprocess.run(
        ["bash", str(PROBE_WRAPPER), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def test_gateway_probe_wrapper_rejects_unknown_pagerduty_severity_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-severity",
        "urgent",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert (
        "--pagerduty-severity must be one of info, warning, error, critical"
        in result.stderr
    )
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_malformed_pagerduty_detail_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-detail",
        "missing-equals",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-detail must use key=value with a non-empty key" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_empty_pagerduty_detail_key_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-detail",
        "=value",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-detail must use key=value with a non-empty key" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_reserved_pagerduty_detail_key_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-detail",
        "status=pass",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-detail key 'status' is reserved" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_duplicate_pagerduty_detail_key_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-detail",
        "runbook=primary",
        "--pagerduty-detail",
        "Runbook=shadow",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-detail key 'Runbook' is duplicated" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_malformed_pagerduty_detail_key_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-detail",
        "bad key=value",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-detail key must use only ASCII" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_missing_wrapper_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-url",
    )

    assert result.returncode == 1
    assert "--pagerduty-url requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_option_shaped_wrapper_value_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-url",
        "--pagerduty-dry-run",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-url requires a value" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_allows_option_shaped_rollback_hook_arg_value(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    hook = tmp_path / "rollback.sh"
    hook.write_text("#!/usr/bin/env sh\nexit 0\n", encoding="utf-8")
    hook.chmod(0o755)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--rollback-hook",
        str(hook),
        "--rollback-hook-arg",
        "--force",
        "--date",
        "2026-02-31",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--date is not a valid calendar date" in result.stderr
    assert "rollback-hook-arg requires a value" not in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_non_https_pagerduty_url_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-routing-key",
        "routing-key",
        "--pagerduty-url",
        "http://events.example.invalid/v1/enqueue",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-url must use https" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_credentialed_pagerduty_url_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-routing-key",
        "routing-key",
        "--pagerduty-url",
        "https://token:secret@events.example.invalid/v1/enqueue",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-url must not include credentials" in result.stderr
    assert "token:secret" not in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_whitespace_pagerduty_url_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-routing-key",
        "routing-key",
        "--pagerduty-url",
        "https://events.example.invalid/v1/enqueue bad",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--pagerduty-url must not include whitespace" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_invalid_drill_date_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--date",
        "2026-02-31",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--date is not a valid calendar date" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_invalid_drill_start_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--start",
        "24:00Z",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--start is not a valid UTC time" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_malformed_drill_end_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--end",
        "10:15",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "--end must use HH:MMZ" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_missing_rollback_hook_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--rollback-hook",
        str(tmp_path / "missing-rollback.sh"),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "rollback hook must exist" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_non_executable_rollback_hook_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    hook = tmp_path / "rollback.sh"
    hook.write_text("#!/usr/bin/env sh\nexit 0\n", encoding="utf-8")
    hook.chmod(0o644)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--rollback-hook",
        str(hook),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "rollback hook must be executable" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_rollback_hook_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    target = tmp_path / "rollback-target.sh"
    target.write_text("#!/usr/bin/env sh\nexit 0\n", encoding="utf-8")
    target.chmod(0o755)
    hook = tmp_path / "rollback.sh"
    hook.symlink_to(target)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--rollback-hook",
        str(hook),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "rollback hook must not be a symlink" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_rollback_hook_parent_before_artifacts(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    real_parent = tmp_path / "real"
    real_parent.mkdir()
    linked_parent = tmp_path / "linked"
    linked_parent.symlink_to(real_parent, target_is_directory=True)
    hook = linked_parent / "rollback.sh"

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--rollback-hook",
        str(hook),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "rollback hook parent must not be a symlink" in result.stderr
    assert not artifact_dir.exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_artifact_dir(
    tmp_path: Path,
) -> None:
    real_dir = tmp_path / "real-artifacts"
    artifact_dir = tmp_path / "artifacts"
    real_dir.mkdir()
    artifact_dir.symlink_to(real_dir, target_is_directory=True)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "probe artifact directory must not be a symlink" in result.stderr
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_artifact_parent(
    tmp_path: Path,
) -> None:
    real_parent = tmp_path / "real-parent"
    linked_parent = tmp_path / "linked-parent"
    real_parent.mkdir()
    linked_parent.symlink_to(real_parent, target_is_directory=True)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(linked_parent / "artifacts"),
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "probe artifact directory parent must not be a symlink" in result.stderr
    assert not (real_parent / "artifacts").exists()
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_explicit_report(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    report_target = tmp_path / "probe-target.json"
    report = tmp_path / "probe.json"
    report_target.write_text("{}", encoding="utf-8")
    report.symlink_to(report_target)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--",
        "--host",
        "gateway.example.invalid",
        "--report-json",
        str(report),
    )

    assert result.returncode == 1
    assert "probe JSON report must not be a symlink" in result.stderr
    assert report_target.read_text(encoding="utf-8") == "{}"
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout


def test_gateway_probe_wrapper_rejects_symlinked_pagerduty_output(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    page_target = tmp_path / "pagerduty-target.json"
    page_output = tmp_path / "pagerduty.json"
    page_target.write_text("{}", encoding="utf-8")
    page_output.symlink_to(page_target)

    result = run_wrapper(
        "--workspace",
        str(REPO_ROOT),
        "--artifact-dir",
        str(artifact_dir),
        "--pagerduty-routing-key",
        "routing-key",
        "--pagerduty-output",
        str(page_output),
        "--pagerduty-dry-run",
        "--",
        "--host",
        "gateway.example.invalid",
    )

    assert result.returncode == 1
    assert "PagerDuty payload must not be a symlink" in result.stderr
    assert page_target.read_text(encoding="utf-8") == "{}"
    assert "Running cargo xtask sorafs-gateway-probe" not in result.stdout
