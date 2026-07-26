"""Tests for the unsigned deterministic SoraFS reference-validator packager."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tarfile
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "package_sorafs_validate_release.sh"
PACKAGE_NAME = "sorafs-validate-test-version-test-target"
TEST_EPOCH = 1_234_567_890


def write_fake_validator(path: Path, body: str = "") -> Path:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        f"{body}\n"
        "if '--help' in sys.argv:\n"
        "    print('deterministic fake validator help')\n",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return path


def run_packager(
    tmp_path: Path,
    fake_binary: Path,
    *,
    out_dir: Path | None = None,
    extra_args: list[str] | None = None,
    env: dict[str, str] | None = None,
    omit_options: set[str] | None = None,
) -> subprocess.CompletedProcess[str]:
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    option_pairs = [
        ("--workspace", str(REPO_ROOT)),
        ("--binary", str(fake_binary)),
        ("--out-dir", str(out_dir or tmp_path / "out")),
        ("--target", "test-target"),
        ("--version", "test-version"),
        ("--source-commit", commit),
        ("--source-date-epoch", str(TEST_EPOCH)),
    ]
    command = ["bash", str(SCRIPT)]
    omitted = omit_options or set()
    for option, value in option_pairs:
        if option not in omitted:
            command.extend([option, value])
    command.append("--skip-smoke")
    command.extend(extra_args or [])
    environment = os.environ.copy()
    environment["SOURCE_DATE_EPOCH"] = str(TEST_EPOCH)
    environment.update(env or {})
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )


def output_paths(root: Path) -> dict[str, Path]:
    return {
        "archive": root / f"{PACKAGE_NAME}.tar.gz",
        "archive_sha": root / f"{PACKAGE_NAME}.tar.gz.sha256",
        "binary_sha": root / f"{PACKAGE_NAME}.sha256",
        "manifest": root / f"{PACKAGE_NAME}.manifest.json",
        "manifest_sha": root / f"{PACKAGE_NAME}.manifest.json.sha256",
    }


def test_release_packager_emits_unsigned_closed_outputs(tmp_path: Path) -> None:
    result = run_packager(tmp_path, write_fake_validator(tmp_path / "sorafs-validate"))
    assert result.returncode == 0, result.stderr

    outputs = output_paths(tmp_path / "out")
    assert all(path.is_file() for path in outputs.values())
    assert not list((tmp_path / "out").glob("*.sig"))
    manifest = json.loads(outputs["manifest"].read_text(encoding="utf-8"))
    assert manifest["schema_version"] == 1
    assert manifest["package"] == "sorafs-validate"
    assert manifest["commit"] == subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    assert manifest["archive"] == f"{PACKAGE_NAME}.tar.gz"
    assert manifest["smoke_checks"] is False
    assert manifest["source_date_epoch"] == TEST_EPOCH
    assert manifest["built_at"] == "2009-02-13T23:31:30Z"
    assert "signature" not in json.dumps(manifest).casefold()


@pytest.mark.parametrize(
    "missing_option",
    ("--target", "--version", "--source-commit", "--source-date-epoch"),
)
def test_release_packager_requires_reviewed_identity_before_outputs(
    tmp_path: Path,
    missing_option: str,
) -> None:
    result = run_packager(
        tmp_path,
        write_fake_validator(tmp_path / "sorafs-validate"),
        omit_options={missing_option},
    )
    assert result.returncode != 0
    assert "package_sorafs_validate_release.sh --target" in result.stderr
    assert not (tmp_path / "out").exists()


@pytest.mark.parametrize(
    "retired_option",
    (
        "--manifest-signing-key",
        "--development-local-signing",
        "--manifest-signature-in",
        "--manifest-public-key",
        "--manifest-public-key-fingerprint",
        "--manifest-signature-out",
    ),
)
def test_release_packager_rejects_retired_signing_options_before_outputs(
    tmp_path: Path,
    retired_option: str,
) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    extra = [retired_option]
    if retired_option != "--development-local-signing":
        extra.append("retired-value")
    result = run_packager(tmp_path, fake, extra_args=extra)
    assert result.returncode != 0
    assert f"unknown argument: {retired_option}" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_replay_is_byte_identical(tmp_path: Path) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first = run_packager(tmp_path, fake, out_dir=first_root)
    second = run_packager(tmp_path, fake, out_dir=second_root)
    assert first.returncode == second.returncode == 0, first.stderr + second.stderr
    for key, first_path in output_paths(first_root).items():
        assert first_path.read_bytes() == output_paths(second_root)[key].read_bytes()


def test_release_packager_archive_has_canonical_metadata_and_order(
    tmp_path: Path,
) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    result = run_packager(tmp_path, fake)
    assert result.returncode == 0, result.stderr
    archive = output_paths(tmp_path / "out")["archive"]
    with tarfile.open(archive, "r:gz") as handle:
        members = handle.getmembers()
        names = [member.name for member in members]
        assert names == sorted(names)
        assert all(member.uid == member.gid == 0 for member in members)
        assert all(member.uname == member.gname == "" for member in members)
        assert all(member.mtime == TEST_EPOCH for member in members)
        assert all(member.mode in {0o644, 0o755} for member in members)


def test_release_packager_builds_locked_target_binary_path(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    cargo_log = tmp_path / "cargo.json"
    fake_cargo = fake_bin / "cargo"
    fake_cargo.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, sys\n"
        "from pathlib import Path\n"
        "args = sys.argv[1:]\n"
        "Path(os.environ['FAKE_CARGO_LOG']).write_text(json.dumps(args))\n"
        "root = Path(args[args.index('--target-dir') + 1])\n"
        "target = args[args.index('--target') + 1]\n"
        "binary = root / target / 'release' / 'sorafs-validate'\n"
        "binary.parent.mkdir(parents=True, exist_ok=True)\n"
        "binary.write_text('#!/bin/sh\\nprintf \"deterministic help\\\\n\"\\n')\n"
        "binary.chmod(0o755)\n",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    environment = os.environ.copy()
    environment["PATH"] = f"{fake_bin}{os.pathsep}{environment['PATH']}"
    environment["FAKE_CARGO_LOG"] = str(cargo_log)
    environment["SOURCE_DATE_EPOCH"] = str(TEST_EPOCH)
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    target_dir = tmp_path / "target"
    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(REPO_ROOT),
            "--out-dir",
            str(tmp_path / "out"),
            "--target",
            "test-target",
            "--target-dir",
            str(target_dir),
            "--version",
            "test-version",
            "--source-commit",
            commit,
            "--source-date-epoch",
            str(TEST_EPOCH),
            "--skip-smoke",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    args = json.loads(cargo_log.read_text(encoding="utf-8"))
    assert args[:3] == ["build", "--locked", "-p"]
    assert args.count("--locked") == 1
    assert (target_dir / "test-target" / "release" / "sorafs-validate").is_file()


def test_release_packager_rejects_symlinked_binary_before_outputs(
    tmp_path: Path,
) -> None:
    target = write_fake_validator(tmp_path / "real-validator")
    link = tmp_path / "validator-link"
    link.symlink_to(target)
    result = run_packager(tmp_path, link)
    assert result.returncode != 0
    assert "must not be a symlink" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_staged_entries(tmp_path: Path) -> None:
    fake = write_fake_validator(
        tmp_path / "sorafs-validate",
        "import os\n"
        "from pathlib import Path\n"
        "if '--help' in sys.argv:\n"
        "    root = Path(os.environ['IROHA_RELEASE_ORIGINAL_EXECUTABLE_ROOT'])\n"
        "    root.joinpath('linked').symlink_to(root / 'sorafs-validate')",
    )
    result = run_packager(tmp_path, fake)
    assert result.returncode != 0
    assert "release inventory entry" in result.stderr
    assert "must not be a symlink" in result.stderr


def test_release_packager_rejects_hardlinked_binary_input(tmp_path: Path) -> None:
    original = write_fake_validator(tmp_path / "original-validator")
    linked = tmp_path / "sorafs-validate"
    os.link(original, linked)
    result = run_packager(tmp_path, linked)
    assert result.returncode != 0
    assert "exactly one hard link" in result.stderr
    assert not output_paths(tmp_path / "out")["archive"].exists()


def test_release_packager_rejects_group_writable_binary_input(
    tmp_path: Path,
) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    fake.chmod(0o775)
    result = run_packager(tmp_path, fake)
    assert result.returncode != 0
    assert "must not be group- or world-writable" in result.stderr
    assert not output_paths(tmp_path / "out")["archive"].exists()


def test_release_packager_rejects_unexpected_staged_regular_file(
    tmp_path: Path,
) -> None:
    fake = write_fake_validator(
        tmp_path / "sorafs-validate",
        "import os\n"
        "from pathlib import Path\n"
        "if '--help' in sys.argv:\n"
        "    root = Path(os.environ['IROHA_RELEASE_ORIGINAL_EXECUTABLE_ROOT'])\n"
        "    root.joinpath('unexpected.txt').write_text('extra')",
    )
    result = run_packager(tmp_path, fake)
    assert result.returncode != 0
    assert "does not exactly match --file entries" in result.stderr
    assert not output_paths(tmp_path / "out")["archive"].exists()


@pytest.mark.parametrize(
    "raw_epoch",
    ("", "-1", "+1", "01", "1.0", "4294967296"),
)
def test_release_packager_rejects_invalid_source_date_epoch_before_outputs(
    tmp_path: Path,
    raw_epoch: str,
) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    result = run_packager(
        tmp_path,
        fake,
        extra_args=["--source-date-epoch", raw_epoch],
    )
    assert result.returncode != 0
    assert (
        "SOURCE_DATE_EPOCH" in result.stderr
        or "--source-date-epoch requires a value" in result.stderr
    )
    assert not (tmp_path / "out").exists()


@pytest.mark.parametrize(
    ("option", "value"),
    (
        ("--version", "bad\nversion"),
        ("--target", "bad/target"),
        ("--profile", "../release"),
    ),
)
def test_release_packager_rejects_control_or_path_tokens_before_outputs(
    tmp_path: Path,
    option: str,
    value: str,
) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    result = run_packager(tmp_path, fake, extra_args=[option, value])
    assert result.returncode != 0
    assert "bounded safe release token" in result.stderr
    assert not (tmp_path / "out").exists()


def test_release_packager_rejects_symlinked_output_root(tmp_path: Path) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    real = tmp_path / "real-out"
    real.mkdir()
    link = tmp_path / "out"
    link.symlink_to(real, target_is_directory=True)
    result = run_packager(tmp_path, fake, out_dir=link)
    assert result.returncode != 0
    assert "release output directory" in result.stderr
    assert "must not be a symlink" in result.stderr
    assert list(real.iterdir()) == []


def test_release_packager_rejects_stale_signature_sidecar(tmp_path: Path) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    out = tmp_path / "out"
    out.mkdir()
    stale = out / f"{PACKAGE_NAME}.manifest.json.sig"
    stale.write_bytes(b"retired")
    result = run_packager(tmp_path, fake, out_dir=out)
    assert result.returncode != 0
    assert "retired package-manifest signature" in result.stderr
    assert stale.read_bytes() == b"retired"


def test_release_packager_checksum_sidecars_match_bytes(tmp_path: Path) -> None:
    fake = write_fake_validator(tmp_path / "sorafs-validate")
    result = run_packager(tmp_path, fake)
    assert result.returncode == 0, result.stderr
    outputs = output_paths(tmp_path / "out")
    for artifact_key, checksum_key in (
        ("archive", "archive_sha"),
        ("manifest", "manifest_sha"),
    ):
        digest, name = outputs[checksum_key].read_text(encoding="utf-8").split()
        assert name == outputs[artifact_key].name
        assert digest == hashlib.sha256(outputs[artifact_key].read_bytes()).hexdigest()


def test_release_packager_source_contains_no_signing_implementation() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    for retired in (
        "manifest_signing_key",
        "manifest_signature_input",
        "development_local_signing",
        "snapshot_release_signing_input",
        "install_manifest_signature",
        "release-manifest --manifest",
        "resolve_release_epoch.py",
        "git describe",
        "rev-parse --short",
    ):
        assert retired not in source
