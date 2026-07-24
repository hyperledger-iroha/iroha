"""Adversarial tests for deterministic SoraFS CLI candidate packaging."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import tarfile

import pytest

from scripts import package_sorafs_cli_candidate as candidate


VERSION = "1.2.3-rc.1"
LINUX_TARGET = "x86_64-unknown-linux-gnu"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"


def _write_executable(path: Path, *, succeeds: bool = True) -> None:
    exit_code = "0" if succeeds else "7"
    path.write_text(
        "#!/bin/sh\n"
        'if [ "${1:-}" = "--help" ]; then printf "candidate help\\n"; fi\n'
        f"exit {exit_code}\n",
        encoding="utf-8",
    )
    path.chmod(0o755)


def _write_candidate(root: Path, *, target: str = LINUX_TARGET) -> None:
    suffix = candidate.TARGET_SUFFIXES[target]
    root.mkdir(parents=True)
    for binary in ("sorafs_cli", "sorafs_fetch", "sorafs-validate"):
        _write_executable(root / f"{binary}{suffix}")
        (root / f"{binary}.help.txt").write_text(
            f"{binary} help\n", encoding="utf-8"
        )
    (root / "version-map.toml").write_text(
        f'release_version = "{VERSION}"\n', encoding="utf-8"
    )
    (root / "ROLLBACK-YANK.md").write_text(
        "# Rollback and yank\nPreserve signed evidence.\n", encoding="utf-8"
    )
    (root / "CHANGELOG.md").write_text("# Changelog\n\n- verified\n", encoding="utf-8")
    (root / "LICENSE").write_text("Test license\n", encoding="utf-8")
    validator = root / "reference-validator"
    validator.mkdir()
    package_name = f"sorafs-validate-{VERSION}-{target}"
    (validator / f"{package_name}.tar.gz").write_bytes(b"deterministic validator")
    (validator / f"{package_name}.tar.gz.sha256").write_text(
        f"{'a' * 64}  {package_name}.tar.gz\n", encoding="utf-8"
    )
    (validator / f"{package_name}.manifest.json").write_text(
        '{"schema_version":1}\n', encoding="utf-8"
    )
    (validator / f"{package_name}.manifest.json.sha256").write_text(
        f"{'b' * 64}  {package_name}.manifest.json\n", encoding="utf-8"
    )
    (validator / f"{package_name}.sha256").write_text(
        f"{'c' * 64}  sorafs-validate{suffix}\n", encoding="utf-8"
    )
    stage = validator / package_name
    stage.mkdir()
    _write_executable(stage / f"sorafs-validate{suffix}")
    (stage / "HELP.txt").write_text("validator help\n", encoding="utf-8")
    include = stage / "include"
    include.mkdir()
    (include / "sorafs_reference.h").write_text(
        "/* checked header */\n", encoding="utf-8"
    )
    (stage / "smoke.advert.json").write_text("{}\n", encoding="utf-8")
    (stage / "smoke.bundle.json").write_text("{}\n", encoding="utf-8")


def _package(
    input_dir: Path,
    output_dir: Path,
    *,
    target: str = LINUX_TARGET,
) -> dict[str, object]:
    return candidate.package_candidate(
        input_dir=input_dir,
        output_dir=output_dir,
        version=VERSION,
        target=target,
    )


def test_candidate_archive_is_reproducible_and_clean_smoked(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    first = _package(input_dir, tmp_path / "first")

    for path in input_dir.rglob("*"):
        if path.is_file():
            os.utime(path, (1_900_000_000, 1_900_000_000))
    second = _package(input_dir, tmp_path / "second")

    assert first["status"] == "verified"
    assert first["clean_smoke_binary_count"] == 3
    assert first["archive_sha256"] == second["archive_sha256"]
    assert first["manifest_sha256"] == second["manifest_sha256"]

    archive_name = str(first["archive"])
    first_archive = tmp_path / "first" / archive_name
    second_archive = tmp_path / "second" / archive_name
    assert first_archive.read_bytes() == second_archive.read_bytes()
    assert hashlib.sha256(first_archive.read_bytes()).hexdigest() == first[
        "archive_sha256"
    ]

    with tarfile.open(first_archive, mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(member.mtime == 0 for member in members)
        assert all(member.uid == 0 and member.gid == 0 for member in members)
        assert all(member.uname == "" and member.gname == "" for member in members)
        manifest_member = archive.extractfile(
            f"sorafs-cli-{VERSION}-{LINUX_TARGET}/PACKAGE-MANIFEST.json"
        )
        assert manifest_member is not None
        manifest = json.load(manifest_member)
    assert manifest["schema"] == candidate.SCHEMA
    assert manifest["payload_file_count"] == first["payload_file_count"]
    assert [row["path"] for row in manifest["files"]] == sorted(
        row["path"] for row in manifest["files"]
    )
    binary_row = next(row for row in manifest["files"] if row["path"] == "sorafs_cli")
    assert binary_row["mode"] == "0755"


def test_candidate_archive_uses_windows_binary_names(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir, target=WINDOWS_TARGET)
    summary = _package(input_dir, tmp_path / "out", target=WINDOWS_TARGET)
    archive = tmp_path / "out" / str(summary["archive"])
    with tarfile.open(archive, mode="r:gz") as package:
        names = {member.name for member in package.getmembers()}
    prefix = f"sorafs-cli-{VERSION}-{WINDOWS_TARGET}"
    assert f"{prefix}/sorafs_cli.exe" in names
    assert f"{prefix}/sorafs_fetch.exe" in names
    assert f"{prefix}/sorafs-validate.exe" in names


def test_candidate_archive_accepts_reference_manifest_signature(
    tmp_path: Path,
) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    package_name = f"sorafs-validate-{VERSION}-{LINUX_TARGET}"
    signature = (
        input_dir
        / "reference-validator"
        / f"{package_name}.manifest.json.sig"
    )
    signature.write_bytes(b"\x01" * 64)

    summary = _package(input_dir, tmp_path / "out")
    archive = tmp_path / "out" / str(summary["archive"])
    with tarfile.open(archive, mode="r:gz") as package:
        assert (
            f"sorafs-cli-{VERSION}-{LINUX_TARGET}/"
            f"reference-validator/{package_name}.manifest.json.sig"
            in {member.name for member in package.getmembers()}
        )


def test_candidate_packager_rejects_symlinked_payload(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    outside = tmp_path / "outside"
    outside.write_text("not candidate evidence\n", encoding="utf-8")
    (input_dir / "evidence-link").symlink_to(outside)
    with pytest.raises(candidate.CandidateError, match="regular file"):
        _package(input_dir, tmp_path / "out")


def test_candidate_packager_rejects_hardlinked_payload(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    os.link(input_dir / "version-map.toml", input_dir / "version-map-copy.toml")
    with pytest.raises(candidate.CandidateError, match="exactly one hard link"):
        _package(input_dir, tmp_path / "out")


def test_candidate_packager_rejects_overlapping_output(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    with pytest.raises(candidate.CandidateError, match="must not overlap"):
        _package(input_dir, input_dir / "dist")


def test_candidate_packager_rejects_payload_mutation_between_scan_and_archive(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    original_write_archive = candidate._write_archive

    def mutate_then_archive(*args: object, **kwargs: object) -> None:
        (input_dir / "version-map.toml").write_text(
            f'release_version = "{VERSION}"\n# mutated\n',
            encoding="utf-8",
        )
        original_write_archive(*args, **kwargs)

    monkeypatch.setattr(candidate, "_write_archive", mutate_then_archive)
    with pytest.raises(candidate.CandidateError, match="changed before archiving"):
        _package(input_dir, tmp_path / "out")


def test_candidate_packager_rejects_missing_release_inventory(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    (input_dir / "ROLLBACK-YANK.md").unlink()
    with pytest.raises(candidate.CandidateError, match="ROLLBACK-YANK.md"):
        _package(input_dir, tmp_path / "out")


def test_candidate_packager_rejects_unexpected_release_inventory(
    tmp_path: Path,
) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    (input_dir / "unreviewed.txt").write_text("unexpected\n", encoding="utf-8")
    with pytest.raises(candidate.CandidateError, match="unexpected release files"):
        _package(input_dir, tmp_path / "out")


def test_candidate_packager_fails_before_publication_when_clean_smoke_fails(
    tmp_path: Path,
) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    _write_executable(input_dir / "sorafs_fetch", succeeds=False)
    output_dir = tmp_path / "out"
    with pytest.raises(candidate.CandidateError, match="clean-consumer smoke failed"):
        _package(input_dir, output_dir)
    assert not list(output_dir.glob("sorafs-cli-*"))


def test_candidate_packager_rejects_unbounded_clean_smoke_output(
    tmp_path: Path,
) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    binary = input_dir / "sorafs_cli"
    binary.write_text(
        "#!/bin/sh\n"
        f"python3 -c 'print(\"x\" * {candidate.MAX_SMOKE_OUTPUT_BYTES + 1})'\n",
        encoding="utf-8",
    )
    binary.chmod(0o755)
    output_dir = tmp_path / "out"
    with pytest.raises(candidate.CandidateError, match="invalid bounded output"):
        _package(input_dir, output_dir)
    assert not list(output_dir.glob("sorafs-cli-*"))


def test_candidate_packager_rejects_symlinked_output_directory(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    actual_output = tmp_path / "actual-output"
    actual_output.mkdir()
    output_link = tmp_path / "output-link"
    output_link.symlink_to(actual_output, target_is_directory=True)
    with pytest.raises(candidate.CandidateError, match="symlink components"):
        _package(input_dir, output_link)


def test_candidate_archive_normalizes_non_binary_file_modes(tmp_path: Path) -> None:
    input_dir = tmp_path / "candidate"
    _write_candidate(input_dir)
    package_name = f"sorafs-validate-{VERSION}-{LINUX_TARGET}"
    source = input_dir / "reference-validator" / package_name / "HELP.txt"
    source.chmod(stat.S_IRUSR | stat.S_IWUSR)
    summary = _package(input_dir, tmp_path / "out")
    archive = tmp_path / "out" / str(summary["archive"])
    member_name = (
        f"sorafs-cli-{VERSION}-{LINUX_TARGET}/"
        f"reference-validator/{package_name}/HELP.txt"
    )
    with tarfile.open(archive, mode="r:gz") as package:
        assert package.getmember(member_name).mode == 0o644
