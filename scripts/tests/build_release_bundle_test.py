from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tarfile
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "build_release_bundle.sh"
VERSION = "2.0.0-rc.2.0"
EPOCH = 1_234_567_890


def _write_executable(path: Path, payload: str) -> Path:
    path.write_text(payload, encoding="utf-8")
    path.chmod(0o755)
    return path


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path, str]:
    binaries = tmp_path / "binaries"
    binaries.mkdir()
    for name in (
        "iroha3d",
        "sorafs_governance_dag",
        "iroha",
        "kagami",
        "attachment_sanitizer",
    ):
        _write_executable(
            binaries / name,
            f"#!/bin/sh\nprintf '%s\\n' {name}\n",
        )
    config = tmp_path / "config"
    config.mkdir()
    (config / "config.toml").write_text("chain = \"test\"\n", encoding="utf-8")
    zstd = _write_executable(
        tmp_path / "zstd",
        "#!/usr/bin/env python3\n"
        "import shutil, sys\n"
        "shutil.copyfileobj(sys.stdin.buffer, sys.stdout.buffer)\n",
    )
    digest = hashlib.sha256(zstd.read_bytes()).hexdigest()
    return binaries, config, zstd, digest


def _run(
    output: Path,
    binaries: Path,
    config: Path,
    zstd: Path,
    digest: str,
    *,
    env: dict[str, str] | None = None,
    omit_options: set[str] | None = None,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment["PATH"] = f"{zstd.parent}{os.pathsep}{environment['PATH']}"
    environment["SOURCE_DATE_EPOCH"] = str(EPOCH)
    environment.update(env or {})
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    option_pairs = [
        ("--profile", "iroha2"),
        ("--config", str(config)),
        ("--target", "x86_64-unknown-linux-gnu"),
        ("--source-commit", commit),
        ("--source-date-epoch", environment["SOURCE_DATE_EPOCH"]),
        ("--prebuilt-bin-dir", str(binaries)),
        ("--artifacts-dir", str(output)),
        ("--zstd", str(zstd)),
        ("--trusted-zstd-sha256", digest),
    ]
    omitted = omit_options or set()
    command = ["bash", str(SCRIPT)]
    for option, value in option_pairs:
        if option not in omitted:
            command.extend([option, value])
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )


@pytest.mark.parametrize(
    "missing_option",
    ("--target", "--source-commit", "--source-date-epoch"),
)
def test_bundle_requires_reviewed_release_identity_before_outputs(
    tmp_path: Path,
    missing_option: str,
) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    output = tmp_path / "out"
    result = _run(
        output,
        binaries,
        config,
        zstd,
        digest,
        omit_options={missing_option},
    )
    assert result.returncode != 0
    assert "Usage:" in result.stderr
    assert not output.exists()


def _outputs(root: Path) -> dict[str, Path]:
    stem = f"iroha2-{VERSION}-linux-x86_64.tar.zst"
    return {
        "archive": root / stem,
        "checksum": root / f"{stem}.sha256",
        "manifest": root / f"iroha2-{VERSION}-linux-x86_64-manifest.json",
    }


def test_bundle_replay_is_byte_identical_and_metadata_normalized(
    tmp_path: Path,
) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first = _run(first_root, binaries, config, zstd, digest)
    second = _run(second_root, binaries, config, zstd, digest)
    assert first.returncode == second.returncode == 0, first.stderr + second.stderr
    for key, first_path in _outputs(first_root).items():
        assert first_path.read_bytes() == _outputs(second_root)[key].read_bytes()

    outputs = _outputs(first_root)
    with tarfile.open(outputs["archive"], "r:") as archive:
        members = archive.getmembers()
        assert [member.name for member in members] == sorted(
            member.name for member in members
        )
        assert all(member.mtime == EPOCH for member in members)
        assert all(member.uid == member.gid == 0 for member in members)
        assert all(member.uname == member.gname == "" for member in members)
        assert all(member.mode in {0o644, 0o755} for member in members)
        bundle_root = f"iroha2-{VERSION}-linux-x86_64"
        expected_binaries = {
            f"{bundle_root}/bin/{name}"
            for name in (
                "iroha3d",
                "sorafs_governance_dag",
                "iroha",
                "kagami",
                "attachment_sanitizer",
            )
        }
        actual_binaries = {
            member.name
            for member in members
            if member.name.startswith(f"{bundle_root}/bin/")
        }
        assert actual_binaries == expected_binaries
    manifest = json.loads(outputs["manifest"].read_text(encoding="utf-8"))
    assert manifest["commit"] and len(manifest["commit"]) == 40
    assert manifest["source_date_epoch"] == EPOCH
    assert manifest["built_at"] == "2009-02-13T23:31:30Z"
    assert manifest["target"] == "x86_64-unknown-linux-gnu"
    assert manifest["compressor"]["sha256"] == digest
    assert manifest["artifacts"][0]["file"] == outputs["archive"].name


def test_bundle_refuses_stale_output_without_replacement(tmp_path: Path) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    output = tmp_path / "out"
    output.mkdir()
    archive = _outputs(output)["archive"]
    archive.write_bytes(b"preserve")
    result = _run(output, binaries, config, zstd, digest)
    assert result.returncode != 0
    assert "refusing stale reuse" in result.stderr
    assert archive.read_bytes() == b"preserve"


def test_bundle_rejects_untrusted_compressor_and_scrubs_archive(
    tmp_path: Path,
) -> None:
    binaries, config, zstd, _ = _fixture(tmp_path)
    output = tmp_path / "out"
    result = _run(output, binaries, config, zstd, "0" * 64)
    assert result.returncode != 0
    assert "zstd executable SHA256 is not trusted" in result.stderr
    assert not _outputs(output)["archive"].exists()


def test_bundle_rejects_compressor_path_replacement_during_launch(
    tmp_path: Path,
) -> None:
    binaries, config, zstd, _ = _fixture(tmp_path)
    zstd.write_text(
        "#!/usr/bin/env python3\n"
        "import os, pathlib, shutil, sys\n"
        "source = pathlib.Path(os.environ['MUTATE_ZSTD_SOURCE'])\n"
        "saved = source.with_name(source.name + '.saved')\n"
        "source.rename(saved)\n"
        "source.write_text('#!/bin/sh\\nexit 99\\n', encoding='utf-8')\n"
        "source.chmod(0o755)\n"
        "source.unlink()\n"
        "saved.rename(source)\n"
        "shutil.copyfileobj(sys.stdin.buffer, sys.stdout.buffer)\n",
        encoding="utf-8",
    )
    zstd.chmod(0o755)
    digest = hashlib.sha256(zstd.read_bytes()).hexdigest()
    output = tmp_path / "out"
    result = _run(
        output,
        binaries,
        config,
        zstd,
        digest,
        env={"MUTATE_ZSTD_SOURCE": str(zstd)},
    )
    assert result.returncode != 0
    assert "zstd executable changed during archive creation" in result.stderr
    assert not _outputs(output)["archive"].exists()


def test_bundle_rejects_hardlinked_prebuilt_binary(tmp_path: Path) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    os.link(binaries / "iroha", tmp_path / "binary-hardlink")
    result = _run(tmp_path / "out", binaries, config, zstd, digest)
    assert result.returncode != 0
    assert "exactly one hard link" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


def test_bundle_rejects_symlinked_config_tree(tmp_path: Path) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    (config / "linked.toml").symlink_to(config / "config.toml")
    result = _run(tmp_path / "out", binaries, config, zstd, digest)
    assert result.returncode != 0
    assert "must not be a symlink" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


@pytest.mark.parametrize("epoch", ("", "-1", "+1", "01", "4294967296"))
def test_bundle_rejects_invalid_epoch(tmp_path: Path, epoch: str) -> None:
    binaries, config, zstd, digest = _fixture(tmp_path)
    result = _run(
        tmp_path / "out",
        binaries,
        config,
        zstd,
        digest,
        env={"SOURCE_DATE_EPOCH": epoch},
    )
    assert result.returncode != 0
    assert (
        "SOURCE_DATE_EPOCH" in result.stderr
        or "--source-date-epoch" in result.stderr
    )
    assert not (tmp_path / "out").exists()


def test_bundle_source_has_no_stale_or_nondeterministic_packaging_paths() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    for marker in (
        "git rev-parse --short",
        "date -u",
        "cp -a",
        'rm -rf "$bundle_dir"',
        "tar -C",
        "sha256sum",
        "shasum",
    ):
        assert marker not in source
    assert "build_release_tar_zst.py" in source
    assert "write_release_checksum.py" in source
    assert "--trusted-zstd-sha256" in source
    assert 'command -v zstd' not in source
    assert "resolve_release_epoch.py" not in source
