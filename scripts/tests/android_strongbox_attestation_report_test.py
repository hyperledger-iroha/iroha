"""Fail-closed tests for Android StrongBox attestation reporting and CI discovery."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
REPORT_PATH = REPO_ROOT / "scripts" / "android_strongbox_attestation_report.py"
CI_PATH = REPO_ROOT / "scripts" / "android_strongbox_attestation_ci.sh"

_SPEC = importlib.util.spec_from_file_location("android_attestation_report", REPORT_PATH)
assert _SPEC is not None and _SPEC.loader is not None
REPORT = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = REPORT
_SPEC.loader.exec_module(REPORT)


def test_report_rejects_missing_bundle_root(tmp_path: Path) -> None:
    report = REPORT.build_report(tmp_path / "missing")

    assert report.has_failures
    assert "No attestation bundles directory" in report.as_text()


def test_report_rejects_empty_bundle_root(tmp_path: Path) -> None:
    root = tmp_path / "bundles"
    root.mkdir()

    report = REPORT.build_report(root)

    assert report.has_failures
    assert "No attestation bundles discovered" in report.as_text()


def _run_empty_ci(tmp_path: Path, bundles_root: Path) -> subprocess.CompletedProcess[str]:
    expectations = tmp_path / "trusted-expectations"
    expectations.mkdir()
    snapshot = tmp_path / "snapshot.txt"
    snapshot.write_text("placeholder\n", encoding="ascii")
    trust_root = tmp_path / "trusted-root.pem"
    trust_root.write_text("placeholder\n", encoding="ascii")
    return subprocess.run(
        [
            "bash",
            str(CI_PATH),
            "--bundles-root",
            str(bundles_root),
            "--summary-out",
            str(tmp_path / "summary.json"),
            "--expectations-root",
            str(expectations),
            "--trust-root",
            str(trust_root),
            "--revocation-snapshot",
            str(snapshot),
            "--revocation-snapshot-sha256",
            "11" * 32,
            "--evaluation-time-ms",
            "1",
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def test_ci_rejects_missing_bundle_root(tmp_path: Path) -> None:
    result = _run_empty_ci(tmp_path, tmp_path / "missing-bundles")

    assert result.returncode != 0
    assert "failing closed" in result.stderr


def test_ci_rejects_empty_bundle_root(tmp_path: Path) -> None:
    bundles = tmp_path / "bundles"
    bundles.mkdir()

    result = _run_empty_ci(tmp_path, bundles)

    assert result.returncode != 0
    assert "failing closed" in result.stderr
