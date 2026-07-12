"""Tests for scripts/package_sorafs_validate_release.sh."""

from __future__ import annotations

import json
import subprocess
import shutil
from pathlib import Path

import pytest


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
    extra_args: list[str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the release packager without invoking Cargo."""

    package_out_dir = out_dir or tmp_path / "out"
    command = [
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
    ]
    if extra_args is not None:
        command.extend(extra_args)
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def require_openssl() -> str:
    """Return the openssl executable path or skip tests requiring it."""

    openssl = shutil.which("openssl")
    if openssl is None:
        pytest.skip("openssl not available")
    return openssl


def write_signing_keypair(tmp_path: Path) -> tuple[Path, Path]:
    """Generate a temporary RSA keypair for manifest signing tests."""

    openssl = require_openssl()
    private_key = tmp_path / "manifest-private.pem"
    public_key = tmp_path / "manifest-public.pem"
    subprocess.run(
        [
            openssl,
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
            str(private_key),
        ],
        cwd=REPO_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )
    subprocess.run(
        [
            openssl,
            "rsa",
            "-in",
            str(private_key),
            "-pubout",
            "-out",
            str(public_key),
        ],
        cwd=REPO_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )
    return private_key, public_key


def test_release_packager_accepts_regular_staged_files(tmp_path: Path) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(tmp_path, fake_binary)

    assert result.returncode == 0, result.stderr
    package = tmp_path / "out" / "sorafs-validate-test-version-test-target"
    assert (tmp_path / "out" / f"{package.name}.tar.gz").is_file()
    assert (tmp_path / "out" / f"{package.name}.tar.gz.sha256").is_file()
    assert (tmp_path / "out" / f"{package.name}.sha256").is_file()
    assert (tmp_path / "out" / f"{package.name}.manifest.json").is_file()
    assert (tmp_path / "out" / f"{package.name}.manifest.json.sha256").is_file()


def test_release_packager_uses_windows_executable_name_for_windows_target(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate.exe",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--target", "x86_64-pc-windows-msvc"],
    )

    assert result.returncode == 0, result.stderr
    manifest_path = (
        tmp_path
        / "out"
        / "sorafs-validate-test-version-x86_64-pc-windows-msvc.manifest.json"
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["binary"] == "sorafs-validate.exe"
    assert manifest["stage_files"][0]["path"] == "sorafs-validate.exe"
    assert (
        tmp_path
        / "out"
        / "sorafs-validate-test-version-x86_64-pc-windows-msvc"
        / "sorafs-validate.exe"
    ).is_file()


def test_release_packager_rejects_missing_option_value_without_shell_error(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(tmp_path, fake_binary, extra_args=["--out-dir"])

    assert result.returncode != 0
    assert "error: --out-dir requires a value" in result.stderr
    assert "unbound variable" not in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_option_shaped_value_before_artifacts(
    tmp_path: Path,
) -> None:
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--out-dir", "--version", "shadow"],
    )

    assert result.returncode != 0
    assert "error: --out-dir requires a value" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_binary_before_artifacts(
    tmp_path: Path,
) -> None:
    target = write_fake_validator(
        tmp_path / "real-sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    binary_link = tmp_path / "sorafs-validate-link"
    binary_link.symlink_to(target)

    result = run_packager(tmp_path, binary_link)

    assert result.returncode != 0
    assert "sorafs-validate binary must not be a symlink" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_manifest_signing_key_before_artifacts(
    tmp_path: Path,
) -> None:
    key_target = tmp_path / "real-key.pem"
    key_link = tmp_path / "key-link.pem"
    key_target.write_text("fixture key", encoding="utf-8")
    key_link.symlink_to(key_target)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=["--manifest-signing-key", str(key_link)],
    )

    assert result.returncode != 0
    assert "manifest signing key must not be a symlink" in result.stderr
    assert key_target.read_text(encoding="utf-8") == "fixture key"
    assert not (tmp_path / "out").exists()


def test_release_packager_writes_manifest_signature_through_hardened_path(
    tmp_path: Path,
) -> None:
    private_key, public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--manifest-public-key",
            str(public_key),
        ],
    )

    assert result.returncode == 0, result.stderr
    package = tmp_path / "out" / "sorafs-validate-test-version-test-target"
    signature = tmp_path / "out" / f"{package.name}.manifest.json.sig"
    assert signature.is_file()
    assert signature.stat().st_size > 0


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
    package = target_out_dir / "sorafs-validate-test-version-test-target"
    package.mkdir()
    sentinel = package / "sentinel.txt"
    sentinel.write_text("keep", encoding="utf-8")
    out_dir = tmp_path / "out-link"
    out_dir.symlink_to(target_out_dir, target_is_directory=True)

    result = run_packager(tmp_path, fake_binary, out_dir=out_dir)

    assert result.returncode != 0
    assert "release output directory" in result.stderr
    assert "out-link" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert sentinel.read_text(encoding="utf-8") == "keep"
    assert not list(target_out_dir.glob("*.tar.gz"))


def test_release_packager_rejects_symlinked_manifest_signature_output(
    tmp_path: Path,
) -> None:
    private_key, _public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    linked_target = tmp_path / "linked-signature-target.sig"
    linked_target.write_text("old", encoding="utf-8")
    signature_link = tmp_path / "signature-link.sig"
    signature_link.symlink_to(linked_target)

    result = run_packager(
        tmp_path,
        fake_binary,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--manifest-signature-out",
            str(signature_link),
        ],
    )

    assert result.returncode != 0
    assert "release manifest signature output" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert linked_target.read_text(encoding="utf-8") == "old"


def test_release_packager_rejects_manifest_signature_overwriting_manifest(
    tmp_path: Path,
) -> None:
    private_key, _public_key = write_signing_keypair(tmp_path)
    fake_binary = write_fake_validator(
        tmp_path / "sorafs-validate",
        "#!/usr/bin/env python3\nprint('fake help')\n",
    )
    out_dir = tmp_path / "out"
    package = out_dir / "sorafs-validate-test-version-test-target"
    manifest_path = out_dir / f"{package.name}.manifest.json"

    result = run_packager(
        tmp_path,
        fake_binary,
        out_dir=out_dir,
        extra_args=[
            "--manifest-signing-key",
            str(private_key),
            "--manifest-signature-out",
            str(manifest_path),
        ],
    )

    assert result.returncode != 0
    assert "release manifest signature output" in result.stderr
    assert "must not overwrite the manifest" in result.stderr
