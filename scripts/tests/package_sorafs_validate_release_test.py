"""Tests for scripts/package_sorafs_validate_release.sh."""

from __future__ import annotations

import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "package_sorafs_validate_release.sh"


def write_fake_validator(path: Path, body: str) -> Path:
    """Write an executable fake sorafs-validate binary."""

    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)
    return path


def run_packager(
    tmp_path: Path,
    fake_binary: Path,
    *,
    out_dir: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the release packager without invoking Cargo."""

    package_out_dir = out_dir or tmp_path / "out"
    return subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(REPO_ROOT),
            "--binary",
            str(fake_binary),
            "--out-dir",
            str(package_out_dir),
            "--target",
            "test-target",
            "--version",
            "test-version",
            "--skip-smoke",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def test_release_packager_accepts_regular_staged_files(tmp_path: Path) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(tmp_path, fake_binary)

    assert result.returncode == 0, result.stderr
    package = tmp_path / "out" / "sorafs-validate-test-version-test-target"
    assert (tmp_path / "out" / f"{package.name}.tar.gz").is_file()
    assert (tmp_path / "out" / f"{package.name}.manifest.json").is_file()


def test_release_packager_rejects_symlinked_staged_entries(tmp_path: Path) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "\n".join(
            [
                "#!/usr/bin/env python3",
                "from pathlib import Path",
                "Path(__file__).resolve().parent.joinpath('symlinked.txt').symlink_to(__file__)",
                "print('fake help')",
                "",
            ]
        ),
    )

    result = run_packager(tmp_path, fake_binary)

    assert result.returncode != 0
    assert "release package entry" in result.stderr
    assert "symlinked.txt" in result.stderr
    assert "must not be a symlink" in result.stderr


def test_release_packager_rejects_symlinked_output_parent_before_archive(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    target_out_dir = tmp_path / "target-out"
    target_out_dir.mkdir()
    out_dir = tmp_path / "out-link"
    out_dir.symlink_to(target_out_dir, target_is_directory=True)

    result = run_packager(tmp_path, fake_binary, out_dir=out_dir)

    assert result.returncode != 0
    assert "release package archive parent" in result.stderr
    assert "out-link" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert not list(target_out_dir.glob("*.tar.gz"))
