"""Adversarial contract tests for the native privacy evidence archive audit."""

from __future__ import annotations

import hashlib
import io
from pathlib import Path
import stat
import subprocess
import sys
import tarfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
AUDITOR = ROOT / "scripts/audit_taira_privacy_native_archive.py"


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _stage(tmp_path: Path) -> Path:
    root = (tmp_path / "stage").resolve()
    (root / "bin").mkdir(parents=True)
    (root / "provenance").mkdir()
    (root / "bin/irohad").write_bytes(b"validator-binary\n")
    (root / "provenance/invocation.json").write_bytes(b'{"schema":"test"}\n')
    files = sorted(path for path in root.rglob("*") if path.is_file())
    (root / "provenance/SHA256SUMS").write_text(
        "".join(
            f"{_digest(path)}  {path.relative_to(root).as_posix()}\n" for path in files
        ),
        encoding="ascii",
    )
    return root


def _normalized_info(
    name: str, path: Path, *, size: int, kind: bytes
) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.type = kind
    info.mode = stat.S_IMODE(path.stat().st_mode)
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    info.size = size
    return info


def _write_archive(
    root: Path,
    archive: Path,
    *,
    replacements: dict[str, bytes] | None = None,
    extras: tuple[tuple[tarfile.TarInfo, bytes | None], ...] = (),
) -> None:
    replacements = replacements or {}
    paths = [root, *sorted(root.rglob("*"))]
    with tarfile.open(archive, "w:gz", format=tarfile.GNU_FORMAT) as bundle:
        for path in paths:
            relative = path.relative_to(root).as_posix()
            name = "." if relative == "." else f"./{relative}"
            if path.is_dir():
                bundle.addfile(
                    _normalized_info(name, path, size=0, kind=tarfile.DIRTYPE)
                )
                continue
            payload = replacements.get(relative, path.read_bytes())
            bundle.addfile(
                _normalized_info(name, path, size=len(payload), kind=tarfile.REGTYPE),
                io.BytesIO(payload),
            )
        for info, payload in extras:
            bundle.addfile(info, None if payload is None else io.BytesIO(payload))


def _regular_extra(
    name: str, payload: bytes = b"unexpected\n"
) -> tuple[tarfile.TarInfo, bytes]:
    info = tarfile.TarInfo(name)
    info.type = tarfile.REGTYPE
    info.mode = 0o600
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    info.size = len(payload)
    return info, payload


def _run(root: Path, archive: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(AUDITOR),
            "--archive",
            str(archive.resolve()),
            "--staged-root",
            str(root),
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def test_accepts_exact_safe_archive_and_post_creation_checksum_closure(
    tmp_path: Path,
) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "evidence.tar.gz").resolve()
    _write_archive(root, archive)

    result = _run(root, archive)

    assert result.returncode == 0, result.stderr
    assert "matches its safe staged checksum closure" in result.stdout


@pytest.mark.parametrize("name", ["/absolute", "../escape", "./bin/../escape"])
def test_rejects_absolute_and_parent_traversal_members(
    tmp_path: Path, name: str
) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "traversal.tar.gz").resolve()
    _write_archive(root, archive, extras=(_regular_extra(name),))

    result = _run(root, archive)

    assert result.returncode != 0
    assert "archive member" in result.stderr


def test_rejects_duplicate_member_names(tmp_path: Path) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "duplicate.tar.gz").resolve()
    duplicate = _regular_extra("./bin/irohad", (root / "bin/irohad").read_bytes())
    _write_archive(root, archive, extras=(duplicate,))

    result = _run(root, archive)

    assert result.returncode != 0
    assert "repeats member bin/irohad" in result.stderr


@pytest.mark.parametrize("kind", [tarfile.SYMTYPE, tarfile.LNKTYPE])
def test_rejects_symbolic_and_hard_link_members(tmp_path: Path, kind: bytes) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "link.tar.gz").resolve()
    info = tarfile.TarInfo("./forbidden-link")
    info.type = kind
    info.mode = 0o777
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    info.linkname = "./bin/irohad"
    _write_archive(root, archive, extras=((info, None),))

    result = _run(root, archive)

    assert result.returncode != 0
    assert "forbidden link" in result.stderr


def test_rejects_special_file_members(tmp_path: Path) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "fifo.tar.gz").resolve()
    info = tarfile.TarInfo("./forbidden-fifo")
    info.type = tarfile.FIFOTYPE
    info.mode = 0o600
    info.uid = 0
    info.gid = 0
    info.mtime = 0
    _write_archive(root, archive, extras=((info, None),))

    result = _run(root, archive)

    assert result.returncode != 0
    assert "forbidden special file" in result.stderr


def test_rejects_archive_content_that_differs_from_stage(tmp_path: Path) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "tampered-archive.tar.gz").resolve()
    _write_archive(root, archive, replacements={"bin/irohad": b"tampered-binary!\n"})

    result = _run(root, archive)

    assert result.returncode != 0
    assert "archive content differs from stage" in result.stderr


def test_rejects_stage_tamper_after_archive_creation(tmp_path: Path) -> None:
    root = _stage(tmp_path)
    archive = (tmp_path / "stale-archive.tar.gz").resolve()
    _write_archive(root, archive)
    (root / "bin/irohad").write_bytes(b"changed-after-archive\n")

    result = _run(root, archive)

    assert result.returncode != 0
    assert "staged checksum mismatch for bin/irohad" in result.stderr
