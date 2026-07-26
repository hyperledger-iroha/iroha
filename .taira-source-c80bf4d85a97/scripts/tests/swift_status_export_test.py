"""Regression tests for ci/swift_status_export.sh."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def test_status_export_regenerates_export_checksum_labels(tmp_path: Path) -> None:
    export_dir = tmp_path / "exports"
    parity_export = export_dir / "renamed-parity-feed.json"
    ci_export = export_dir / "renamed-ci-feed.json"
    tmp_dir = tmp_path / "tmp"
    tmp_dir.mkdir()

    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("SWIFT_") and key != "MOBILE_PARITY_PIPELINE_METADATA"
    }
    env.update(
        {
            "TMPDIR": str(tmp_dir),
            "SWIFT_STATUS_DISABLE_HEALTH_OUTPUT": "1",
            "SWIFT_STATUS_DISABLE_METRICS": "1",
            "SWIFT_STATUS_EXPORT_OUT": str(tmp_path / "weekly_digest.md"),
            "SWIFT_STATUS_SUMMARY_OUT": str(tmp_path / "summary.json"),
            "SWIFT_PARITY_FEED_EXPORT_PATH": str(parity_export),
            "SWIFT_CI_FEED_EXPORT_PATH": str(ci_export),
        }
    )

    completed = subprocess.run(
        ["bash", "ci/swift_status_export.sh"],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr

    for exported in (parity_export, ci_export):
        sidecar = exported.with_name(exported.name + ".sha256")
        assert sidecar.is_file()
        checksum_fields = sidecar.read_text(encoding="utf-8").split()
        assert checksum_fields[-1] == exported.name

    checker = subprocess.run(
        [
            sys.executable,
            "scripts/check_swift_dashboard_data.py",
            "--require-checksum-sidecars",
            str(parity_export),
            str(ci_export),
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert checker.returncode == 0, checker.stdout + checker.stderr
