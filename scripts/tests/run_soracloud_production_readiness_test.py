"""Focused execution tests for the Soracloud production-readiness runner."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "scripts/ci/run_soracloud_production_readiness.sh"


def test_config_fixture_gate_uses_the_consolidated_integration_target() -> None:
    source = RUNNER.read_text(encoding="utf-8")

    assert "--test iroha_config_integration fixtures::" in source
    assert "--test fixtures" not in source


def test_retired_allow_open_blockers_flag_is_rejected() -> None:
    result = subprocess.run(
        ["/bin/bash", str(RUNNER), "--allow-open-blockers"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )

    assert result.returncode == 2
    assert "ERROR: unknown option: --allow-open-blockers" in result.stderr
    assert "--allow-open-blockers" not in result.stderr.removeprefix(
        "ERROR: unknown option: --allow-open-blockers\n"
    )
    assert "ALLOW_OPEN_BLOCKERS" not in RUNNER.read_text(encoding="utf-8")


def test_focused_success_reports_only_the_local_profile(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    fake_bash = fake_bin / "bash"
    fake_bash.write_text(
        """#!/bin/bash
if [ "$1" = "-lc" ]; then
  case "$2" in
    *"cargo test "*) exit 0 ;;
    *) exec /bin/bash -c "$2" ;;
  esac
fi
exec /bin/bash "$@"
""",
        encoding="utf-8",
    )
    fake_bash.chmod(0o755)
    out_dir = tmp_path / "report"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"

    result = subprocess.run(
        [
            "/bin/bash",
            str(RUNNER),
            "--profile",
            "focused",
            "--out",
            str(out_dir),
            "--step-timeout-seconds",
            "0",
        ],
        cwd=ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
        timeout=20,
    )

    assert result.returncode == 0
    report = (out_dir / "soracloud_production_readiness.md").read_text(
        encoding="utf-8"
    )
    assert (
        "## Summary\n\n"
        "Local Soracloud/Inrou readiness profile `focused` passed.\n\n"
        "Native Linux/KVM four-peer qualification and public Taira cutover are "
        "separate evidence gates; this local runner does not execute or satisfy them."
    ) in report
    assert "All required Soracloud production readiness gates passed" not in report


def test_blocked_gate_always_exits_nonzero(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    fake_bash = fake_bin / "bash"
    fake_bash.write_text(
        """#!/bin/bash
if [ "$1" = "-lc" ]; then
  case "$2" in
    *"cargo test "*) exit 0 ;;
    *) exec /bin/bash -c "$2" ;;
  esac
fi
exec /bin/bash "$@"
""",
        encoding="utf-8",
    )
    fake_bash.chmod(0o755)
    out_dir = tmp_path / "report"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"

    result = subprocess.run(
        [
            "/bin/bash",
            str(RUNNER),
            "--profile",
            "full",
            "--out",
            str(out_dir),
            "--step-timeout-seconds",
            "0",
            "--skip-portable",
            "--skip-integration",
        ],
        cwd=ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
        timeout=20,
    )

    assert result.returncode == 1
    report = (out_dir / "soracloud_production_readiness.md").read_text(
        encoding="utf-8"
    )
    assert "production readiness is blocked by missing required gates" in report
    assert "### Open Blockers" in report
    assert "Allow open blockers" not in report
