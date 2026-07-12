"""Tests for the explicit Taira v2 24-hour soak launcher."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "run_taira_v2_24h_soak.sh"


def _stubbed_environment(tmp_path: Path) -> tuple[dict[str, str], Path]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    capture = tmp_path / "cargo-invocation.txt"
    cargo = bin_dir / "cargo"
    cargo.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "printf '%s\\n' \"duration=$IROHA_TAIRA_SIM_DURATION_SECS\" "
        "\"loss=$IROHA_TAIRA_PACKET_LOSS_PERCENT\" "
        "\"churn=$IROHA_TAIRA_CHURN_INTERVAL_SECS\" "
        "\"args=$*\" > \"$TAIRA_SOAK_CAPTURE\"\n",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TAIRA_SOAK_CAPTURE"] = str(capture)
    return env, capture


def test_launcher_exports_profile_and_selects_ignored_soak(tmp_path: Path) -> None:
    env, capture = _stubbed_environment(tmp_path)
    result = subprocess.run(
        [
            str(SCRIPT),
            "--duration-secs",
            "30",
            "--packet-loss-percent",
            "17",
            "--churn-interval-secs",
            "30",
        ],
        cwd=REPO_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    invocation = capture.read_text(encoding="utf-8")
    assert "duration=30" in invocation
    assert "loss=17" in invocation
    assert "churn=30" in invocation
    assert "-p integration_tests --test consensus_and_da" in invocation
    assert "taira_profile_24h_packet_impairment_and_restart_soak" in invocation
    assert "--ignored --nocapture" in invocation


def test_launcher_rejects_invalid_packet_loss_before_cargo(tmp_path: Path) -> None:
    env, capture = _stubbed_environment(tmp_path)
    result = subprocess.run(
        [str(SCRIPT), "--packet-loss-percent", "101"],
        cwd=REPO_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "packet loss" in result.stderr
    assert not capture.exists()
