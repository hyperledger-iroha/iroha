from __future__ import annotations

import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import release_artifact_contract as contract


@pytest.mark.parametrize(
    "raw",
    ("", "-1", "+1", "01", " 1", "1 ", "1.0", str(contract.MAX_SOURCE_DATE_EPOCH + 1)),
)
def test_source_date_epoch_rejects_noncanonical_or_unsupported_values(
    raw: str,
) -> None:
    with pytest.raises(contract.ReleaseArtifactError):
        contract.parse_source_date_epoch(raw)


def test_source_date_epoch_renders_canonical_rfc3339() -> None:
    assert contract.parse_source_date_epoch("0") == 0
    assert contract.format_source_date_epoch(0) == "1970-01-01T00:00:00Z"
    assert contract.format_source_date_epoch(1) == "1970-01-01T00:00:01Z"


def test_stable_hash_rejects_hardlinked_files(tmp_path: Path) -> None:
    root = tmp_path / "root"
    root.mkdir()
    artifact = root / "artifact.bin"
    artifact.write_bytes(b"bytes")
    os.link(artifact, root / "copy.bin")
    with pytest.raises(contract.ReleaseArtifactError, match="exactly one hard link"):
        contract.stable_hash_relative(root, "artifact.bin")


def test_create_fresh_directory_rejects_links_and_existing_outputs(
    tmp_path: Path,
) -> None:
    created = contract.create_fresh_directory(tmp_path / "new" / "release")
    assert created == tmp_path / "new" / "release"
    assert created.is_dir()
    assert created.stat().st_mode & 0o777 == 0o755
    with pytest.raises(contract.ReleaseArtifactError, match="already exists"):
        contract.create_fresh_directory(created)

    linked_parent = tmp_path / "linked"
    linked_parent.symlink_to(tmp_path / "new", target_is_directory=True)
    with pytest.raises(contract.ReleaseArtifactError, match="not a directory"):
        contract.create_fresh_directory(linked_parent / "other")


def test_create_fresh_directory_rejects_writable_ancestor(tmp_path: Path) -> None:
    unsafe = tmp_path / "unsafe"
    unsafe.mkdir(mode=0o755)
    unsafe.chmod(0o775)
    with pytest.raises(contract.ReleaseArtifactError, match="group- or world-writable"):
        contract.create_fresh_directory(unsafe / "release")


def test_json_loader_rejects_duplicate_keys() -> None:
    with pytest.raises(contract.ReleaseArtifactError, match="duplicate JSON object key"):
        contract.load_json_object(b'{"a":1,"a":2}', "fixture")


def test_exclusive_output_scrubs_hardlink_race(tmp_path: Path) -> None:
    output = tmp_path / "output.bin"
    leaked = tmp_path / "hardlink.bin"
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was written"):
        with contract.exclusive_output_fd(output) as descriptor:
            os.write(descriptor, b"release-secret")
            os.link(output, leaked)
    assert not output.exists()
    assert leaked.read_bytes() == b""


def test_exclusive_output_rejects_target_substitution_without_deleting_it(
    tmp_path: Path,
) -> None:
    output = tmp_path / "output.bin"
    moved = tmp_path / "moved.bin"
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was written"):
        with contract.exclusive_output_fd(output) as descriptor:
            os.write(descriptor, b"release-secret")
            output.rename(moved)
            output.write_bytes(b"attacker")
    assert output.read_bytes() == b"attacker"
    assert moved.read_bytes() == b""


def test_exclusive_output_rejects_parent_substitution(tmp_path: Path) -> None:
    parent = tmp_path / "parent"
    parent.mkdir()
    output = parent / "output.bin"
    moved_parent = tmp_path / "moved-parent"
    with pytest.raises(contract.ReleaseArtifactError, match="directory .* was replaced"):
        with contract.exclusive_output_fd(output) as descriptor:
            os.write(descriptor, b"release-secret")
            parent.rename(moved_parent)
            parent.mkdir()
    assert not (moved_parent / "output.bin").exists()
    assert list(parent.iterdir()) == []


def test_exclusive_output_rejects_permission_mutation(tmp_path: Path) -> None:
    output = tmp_path / "output.bin"
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was written"):
        with contract.exclusive_output_fd(output, mode=0o644) as descriptor:
            os.write(descriptor, b"release-secret")
            output.chmod(0o666)
    assert not output.exists()


def test_stable_read_rejects_path_replacement_during_read(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "root"
    root.mkdir()
    artifact = root / "artifact.bin"
    artifact.write_bytes(b"a" * (2 * 1024 * 1024))
    replaced = root / "replaced.bin"
    original_read = contract.os.read
    raced = False

    def replace_after_first_read(descriptor: int, size: int) -> bytes:
        nonlocal raced
        chunk = original_read(descriptor, size)
        if chunk and not raced:
            raced = True
            artifact.rename(replaced)
            artifact.write_bytes(b"b")
        return chunk

    monkeypatch.setattr(contract.os, "read", replace_after_first_read)
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was read"):
        contract.stable_hash_relative(root, "artifact.bin")


def test_release_manifest_reader_requires_exact_canonical_bytes() -> None:
    manifest = {
        "schema": contract.RELEASE_MANIFEST_SCHEMA,
        "schema_version": contract.RELEASE_MANIFEST_SCHEMA_VERSION,
        "version": "1.0.0",
        "commit": "a" * 40,
        "source_date_epoch": 0,
        "built_at": "1970-01-01T00:00:00Z",
        "os": "linux",
        "arch": "x86_64",
        "artifacts": [
            {
                "profile": "iroha3",
                "target": "x86_64-unknown-linux-gnu",
                "kind": "bundle",
                "format": "tar.zst",
                "path": "iroha3.tar.zst",
                "sha256": "b" * 64,
                "size": 1,
            }
        ],
    }
    canonical = contract.canonical_json_bytes(manifest)
    assert contract.load_canonical_release_manifest(canonical) == manifest
    compact = (
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    with pytest.raises(contract.ReleaseArtifactError, match="not in canonical"):
        contract.load_canonical_release_manifest(compact)


def test_stable_open_rechecks_consumed_content_when_timestamps_collide(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifact = tmp_path / "artifact.bin"
    original = b"original content"
    artifact.write_bytes(original)
    expected = contract.stable_hash_path(artifact)
    real_stat, real_fstat = os.stat, os.fstat

    def colliding_times(info):
        if (info.st_dev, info.st_ino) != (expected.device, expected.inode):
            return info
        fields = {name: getattr(info, name) for name in dir(info) if name.startswith("st_")}
        return SimpleNamespace(**(fields | {
            "st_mtime_ns": expected.mtime_ns,
            "st_ctime_ns": expected.ctime_ns,
        }))

    # Deterministically model an allowed filesystem timestamp collision; preserve
    # every other identity field. The original source bytes are already consumed.
    monkeypatch.setattr(contract.os, "stat", lambda *a, **k: colliding_times(real_stat(*a, **k)))
    monkeypatch.setattr(contract.os, "fstat", lambda *a, **k: colliding_times(real_fstat(*a, **k)))
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was streamed"):
        with contract.stable_open_relative(tmp_path, artifact.name, expected=expected) as fd:
            assert os.read(fd, expected.size) == original
            artifact.write_bytes(b"x" * expected.size)
            assert artifact.stat().st_mtime_ns == expected.mtime_ns
            assert artifact.stat().st_ctime_ns == expected.ctime_ns
    assert artifact.read_bytes() != original


def test_stable_open_final_hash_preserves_offset_and_bounds_reads(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifact = tmp_path / "artifact.bin"
    payload = b"a" * (2 * 1024 * 1024 + 17)
    artifact.write_bytes(payload)
    expected = contract.stable_hash_path(artifact)
    original_pread = os.pread
    requests = []

    def bounded_read(fd: int, size: int, offset: int) -> bytes:
        assert os.lseek(fd, 0, os.SEEK_CUR) == 3
        chunk = original_pread(fd, size, offset)
        assert os.lseek(fd, 0, os.SEEK_CUR) == 3
        requests.append((size, offset, len(chunk)))
        return chunk

    monkeypatch.setattr(contract.os, "pread", bounded_read)
    with contract.stable_open_relative(tmp_path, artifact.name, expected=expected) as fd:
        assert os.read(fd, 3) == payload[:3]
    assert all(0 < size <= 1024 * 1024 for size, _, _ in requests)
    assert sum(length for _, _, length in requests) == expected.size
    assert requests[-1] == (1, expected.size, 0)
    assert sum(size for size, _, _ in requests) == expected.size + 1


@pytest.mark.parametrize("replacement", (b"", b"short", b"x" * 64))
def test_stable_open_rejects_truncation_or_growth_with_bounded_final_reads(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    replacement: bytes,
) -> None:
    artifact = tmp_path / "artifact.bin"
    artifact.write_bytes(b"original content")
    expected = contract.stable_hash_path(artifact)
    original_pread = os.pread
    requests = []

    def bounded_read(fd: int, size: int, offset: int) -> bytes:
        requests.append((size, offset))
        return original_pread(fd, size, offset)

    monkeypatch.setattr(contract.os, "pread", bounded_read)
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was streamed"):
        with contract.stable_open_relative(tmp_path, artifact.name, expected=expected) as fd:
            assert os.read(fd, expected.size) == b"original content"
            artifact.write_bytes(replacement)
    assert requests
    assert all(0 < size <= 1024 * 1024 and offset + size <= expected.size + 1
               for size, offset in requests)
    if len(replacement) > expected.size:
        assert sum(size for size, _ in requests) == expected.size + 1


@pytest.mark.parametrize("mutation", ("mode", "path"))
def test_stable_open_keeps_metadata_and_path_checks_after_final_hash(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
) -> None:
    artifact = tmp_path / "artifact.bin"
    payload = b"original content"
    artifact.write_bytes(payload)
    expected = contract.stable_hash_path(artifact)
    original_pread = os.pread
    changed = False

    def mutate_after_hash(fd: int, size: int, offset: int) -> bytes:
        nonlocal changed
        chunk = original_pread(fd, size, offset)
        if offset == expected.size and size == 1 and not chunk:
            changed = True
            if mutation == "mode":
                artifact.chmod(0o400)
            else:
                replacement = artifact.with_suffix(".next")
                replacement.write_bytes(payload)
                replacement.chmod(expected.mode)
                os.replace(replacement, artifact)
        return chunk

    monkeypatch.setattr(contract.os, "pread", mutate_after_hash)
    with pytest.raises(contract.ReleaseArtifactError, match="changed while it was streamed"):
        with contract.stable_open_relative(tmp_path, artifact.name, expected=expected) as fd:
            assert os.read(fd, expected.size) == payload
    assert changed
