"""Tests for the SoraFS CLI release wrapper."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "release_sorafs_cli.sh"


def write_inputs(tmp_path: Path) -> tuple[Path, Path, Path]:
    """Create minimal release input files."""

    manifest = tmp_path / "manifest.to"
    chunk_plan = tmp_path / "chunk_plan.json"
    chunk_summary = tmp_path / "chunk_summary.json"
    manifest.write_bytes(b"manifest")
    chunk_plan.write_text("{}", encoding="utf-8")
    chunk_summary.write_text("{}", encoding="utf-8")
    return manifest, chunk_plan, chunk_summary


def run_wrapper(
    tmp_path: Path,
    *extra_args: str,
) -> subprocess.CompletedProcess[str]:
    """Run the release wrapper with a CLI that must not be reached."""

    manifest, chunk_plan, chunk_summary = write_inputs(tmp_path)
    return subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(tmp_path),
            "--manifest",
            str(manifest),
            "--chunk-plan",
            str(chunk_plan),
            "--chunk-summary",
            str(chunk_summary),
            "--cli",
            "/usr/bin/true",
            *extra_args,
        ],
        cwd=REPO_ROOT,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
        check=False,
    )


def test_release_wrapper_defaults_manifest_under_workspace_before_artifacts(
    tmp_path: Path,
) -> None:
    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(tmp_path),
            "--cli",
            "/usr/bin/true",
        ],
        cwd=REPO_ROOT,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
        check=False,
    )

    expected = tmp_path / "fixtures" / "sorafs_manifest" / "ci_sample" / "manifest.to"
    assert result.returncode == 1
    assert f"manifest input not found at {expected}" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_symlinked_bundle_output(tmp_path: Path) -> None:
    target = tmp_path / "bundle-target.json"
    bundle = tmp_path / "manifest.bundle.json"
    target.write_text("{}", encoding="utf-8")
    bundle.symlink_to(target)

    result = run_wrapper(tmp_path, "--bundle-out", str(bundle))

    assert result.returncode == 1
    assert "release bundle output must not be a symlink" in result.stderr
    assert target.read_text(encoding="utf-8") == "{}"


def test_release_wrapper_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    result = run_wrapper(tmp_path, "--bundle-out")

    assert result.returncode == 1
    assert "error: --bundle-out requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_option_shaped_output_value_before_artifacts(
    tmp_path: Path,
) -> None:
    result = run_wrapper(
        tmp_path,
        "--bundle-out",
        "--signature-out",
        str(tmp_path / "manifest.sig"),
    )

    assert result.returncode == 1
    assert "error: --bundle-out requires a value" in result.stderr
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_symlinked_manifest_input_before_artifacts(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target-manifest.to"
    manifest = tmp_path / "manifest-link.to"
    target.write_bytes(b"manifest")
    manifest.symlink_to(target)

    result = run_wrapper(tmp_path, "--manifest", str(manifest))

    assert result.returncode == 1
    assert "manifest input must not be a symlink" in result.stderr
    assert target.read_bytes() == b"manifest"
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_chunk_summary_parent_symlink_before_artifacts(
    tmp_path: Path,
) -> None:
    real_parent = tmp_path / "real-summary-dir"
    linked_parent = tmp_path / "linked-summary-dir"
    real_parent.mkdir()
    summary = real_parent / "chunk_summary.json"
    summary.write_text("{}", encoding="utf-8")
    linked_parent.symlink_to(real_parent, target_is_directory=True)

    result = run_wrapper(
        tmp_path,
        "--chunk-summary",
        str(linked_parent / "chunk_summary.json"),
    )

    assert result.returncode == 1
    assert "chunk summary input parent must not be a symlink" in result.stderr
    assert summary.read_text(encoding="utf-8") == "{}"
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_symlinked_identity_token_file_before_artifacts(
    tmp_path: Path,
) -> None:
    target = tmp_path / "token-target.jwt"
    token_file = tmp_path / "token.jwt"
    target.write_text("token", encoding="utf-8")
    token_file.symlink_to(target)

    result = run_wrapper(tmp_path, "--identity-token-file", str(token_file))

    assert result.returncode == 1
    assert "identity token file must not be a symlink" in result.stderr
    assert target.read_text(encoding="utf-8") == "token"
    assert not (tmp_path / "artifacts").exists()


def test_release_wrapper_rejects_symlinked_signature_parent(tmp_path: Path) -> None:
    real_parent = tmp_path / "real-signature-dir"
    linked_parent = tmp_path / "linked-signature-dir"
    real_parent.mkdir()
    linked_parent.symlink_to(real_parent, target_is_directory=True)

    result = run_wrapper(
        tmp_path,
        "--signature-out",
        str(linked_parent / "manifest.sig"),
    )

    assert result.returncode == 1
    assert "release signature output parent must not be a symlink" in result.stderr
    assert not (real_parent / "manifest.sig").exists()


def test_release_wrapper_rejects_symlinked_sign_summary(tmp_path: Path) -> None:
    output_root = tmp_path / "artifacts" / "sorafs_cli_release"
    output_root.mkdir(parents=True)
    target = tmp_path / "sign-summary-target.json"
    sign_summary = output_root / "manifest.sign.summary.json"
    target.write_text("{}", encoding="utf-8")
    sign_summary.symlink_to(target)

    result = run_wrapper(tmp_path)

    assert result.returncode == 1
    assert "sign summary must not be a symlink" in result.stderr
    assert target.read_text(encoding="utf-8") == "{}"


def test_release_wrapper_rejects_symlinked_verify_summary(tmp_path: Path) -> None:
    output_root = tmp_path / "artifacts" / "sorafs_cli_release"
    output_root.mkdir(parents=True)
    target = tmp_path / "verify-summary-target.json"
    verify_summary = output_root / "manifest.verify.summary.json"
    target.write_text("{}", encoding="utf-8")
    verify_summary.symlink_to(target)

    result = run_wrapper(tmp_path)

    assert result.returncode == 1
    assert "verify summary must not be a symlink" in result.stderr
    assert target.read_text(encoding="utf-8") == "{}"
