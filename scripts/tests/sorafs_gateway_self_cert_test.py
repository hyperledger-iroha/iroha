"""Tests for the SoraFS gateway self-certification wrapper."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "sorafs_gateway_self_cert.sh"


def install_fake_cargo(tmp_path: Path) -> dict[str, str]:
    """Install a fake cargo that makes wrapper preflight tests deterministic."""

    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    cargo = fake_bin / "cargo"
    cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--list" ]]; then
  echo "xtask"
  exit 0
fi
if [[ "${1:-}" == "xtask" ]]; then
  exit 0
fi
echo "unexpected cargo invocation: $*" >&2
exit 127
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    return env


def write_file(path: Path, content: str = "{}") -> Path:
    """Write a small regular file and return it."""

    path.write_text(content, encoding="utf-8")
    return path


def run_wrapper(
    tmp_path: Path,
    *extra_args: str,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the self-cert wrapper."""

    return subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(tmp_path),
            "--signing-key",
            str(write_file(tmp_path / "signing-key.hex", "00")),
            "--signer",
            "admin@operator",
            *extra_args,
        ],
        cwd=REPO_ROOT,
        env=env or os.environ.copy(),
        text=True,
        capture_output=True,
        check=False,
    )


def test_gateway_self_cert_rejects_symlinked_output_dir(tmp_path: Path) -> None:
    target = tmp_path / "real-output"
    output_dir = tmp_path / "attest-output"
    target.mkdir()
    output_dir.symlink_to(target, target_is_directory=True)

    result = run_wrapper(tmp_path, "--out", str(output_dir))

    assert result.returncode == 1
    assert "gateway self-cert output directory must not be a symlink" in result.stderr


def test_gateway_self_cert_rejects_symlinked_output_parent(tmp_path: Path) -> None:
    real_parent = tmp_path / "real-parent"
    linked_parent = tmp_path / "linked-parent"
    real_parent.mkdir()
    linked_parent.symlink_to(real_parent, target_is_directory=True)

    result = run_wrapper(tmp_path, "--out", str(linked_parent / "attest"))

    assert result.returncode == 1
    assert (
        "gateway self-cert output directory parent must not be a symlink"
        in result.stderr
    )
    assert not (real_parent / "attest").exists()


def test_gateway_self_cert_rejects_symlinked_verify_summary(
    tmp_path: Path,
) -> None:
    env = install_fake_cargo(tmp_path)
    output_dir = tmp_path / "attest"
    output_dir.mkdir()
    summary_target = tmp_path / "verify-target.json"
    verify_summary = output_dir / "manifest.verify.summary.json"
    write_file(summary_target)
    verify_summary.symlink_to(summary_target)
    manifest = write_file(tmp_path / "manifest.to")
    bundle = write_file(tmp_path / "manifest.bundle.json")
    chunk_plan = write_file(tmp_path / "chunk_plan.json")
    chunk_summary = write_file(tmp_path / "chunk_summary.json")

    result = run_wrapper(
        tmp_path,
        "--out",
        str(output_dir),
        "--manifest",
        str(manifest),
        "--manifest-bundle",
        str(bundle),
        "--chunk-plan",
        str(chunk_plan),
        "--chunk-summary",
        str(chunk_summary),
        "--cli",
        "/usr/bin/true",
        env=env,
    )

    assert result.returncode == 1
    assert "manifest verification summary must not be a symlink" in result.stderr
    assert summary_target.read_text(encoding="utf-8") == "{}"


def test_gateway_self_cert_rejects_symlinked_denylist_report(
    tmp_path: Path,
) -> None:
    env = install_fake_cargo(tmp_path)
    report_target = tmp_path / "denylist-target.json"
    report = tmp_path / "denylist-report.json"
    write_file(report_target)
    report.symlink_to(report_target)
    old_bundle = write_file(tmp_path / "old-denylist.json")
    new_bundle = write_file(tmp_path / "new-denylist.json")

    result = run_wrapper(
        tmp_path,
        "--out",
        str(tmp_path / "attest"),
        "--denylist-old",
        str(old_bundle),
        "--denylist-new",
        str(new_bundle),
        "--denylist-report",
        str(report),
        "--cli",
        "/usr/bin/true",
        env=env,
    )

    assert result.returncode == 1
    assert "denylist diff report must not be a symlink" in result.stderr
    assert report_target.read_text(encoding="utf-8") == "{}"
