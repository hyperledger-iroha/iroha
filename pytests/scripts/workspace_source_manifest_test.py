"""Tests for the release source-manifest helper."""

from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import socket
import stat
import struct
import subprocess
import tempfile

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "compute_workspace_source_manifest.py"


def load_module():
    spec = importlib.util.spec_from_file_location("workspace_source_manifest", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def init_release_repo(path: Path) -> None:
    subprocess.run(["git", "init", "-q"], cwd=path, check=True)
    subprocess.run(
        ["git", "config", "user.email", "release-test@example.invalid"],
        cwd=path,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Release Test"], cwd=path, check=True
    )
    (path / ".gitignore").write_text("Cargo.lock\ntarget/\n", encoding="utf-8")
    (path / "tracked.txt").write_text("source\n", encoding="utf-8")
    subprocess.run(
        ["git", "add", ".gitignore", "tracked.txt"], cwd=path, check=True
    )
    subprocess.run(
        ["git", "commit", "-qm", "fixture"], cwd=path, check=True
    )
    (path / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _seal_bytes(module, records: list[tuple[bytes, bytes, int, bytes]]) -> bytes:
    payload = bytearray(module._SOURCE_SEAL_DOMAIN)
    payload.extend(struct.pack(">Q", len(records)))
    for member, kind, mode, contents in records:
        payload.extend(struct.pack(">Q", len(member)))
        payload.extend(member)
        payload.extend(kind)
        payload.extend(struct.pack(">I", mode))
        payload.extend(struct.pack(">Q", len(contents)))
        payload.extend(contents)
    return bytes(payload)


def _source_seal_fixture(tmp_path: Path):
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "bin").mkdir()
    executable = source / "bin" / "runner"
    executable.write_bytes(b"#!/bin/sh\nexit 0\n")
    executable.chmod(0o751)
    (source / "payload").write_bytes(b"privacy-release-input\n")
    (source / "link").symlink_to("payload")
    (source / "empty").mkdir()
    paths = [
        "bin/runner",
        "deleted",
        "empty",
        "link",
        "payload",
    ]
    path_list = tmp_path / "source-paths.bin"
    module.write_source_path_list(path_list, paths)
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    archive = tmp_path / "source.seal"
    archive_sha = module.create_source_seal(
        source, path_list, archive, manifest
    )
    assert archive_sha == _sha256(archive)
    return module, source, path_list, manifest, archive, archive_sha


def test_manifest_is_order_independent_and_content_sensitive(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "a.txt").write_text("alpha\n", encoding="utf-8")
    (tmp_path / "b.txt").write_text("beta\n", encoding="utf-8")

    first = module._manifest_for_paths(tmp_path, ["b.txt", "a.txt"])
    assert first == module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])
    path_list = tmp_path / "source-paths.bin"
    module.write_source_path_list(path_list, ["b.txt", "a.txt"])
    assert module.read_source_path_list(path_list) == ["a.txt", "b.txt"]
    assert (
        module.workspace_source_manifest_from_path_list(tmp_path, path_list)
        == first
    )

    (tmp_path / "a.txt").write_text("changed\n", encoding="utf-8")
    assert first != module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])
    assert first != module.workspace_source_manifest_from_path_list(
        tmp_path, path_list
    )

    malformed = tmp_path / "malformed-source-paths.bin"
    malformed.write_bytes(path_list.read_bytes() + b"trailing")
    with pytest.raises(module.SourcePathListError, match="trailing bytes"):
        module.read_source_path_list(malformed)
    symlinked = tmp_path / "symlinked-source-paths.bin"
    symlinked.symlink_to(path_list.name)
    with pytest.raises(module.SourcePathListError, match="regular file"):
        module.read_source_path_list(symlinked)
    with pytest.raises(FileNotFoundError):
        module.read_source_path_list(tmp_path / "missing-source-paths.bin")
    with pytest.raises(FileExistsError):
        module.write_source_path_list(path_list, ["a.txt"])


def test_manifest_distinguishes_deleted_and_symlink_entries(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "target-a").write_text("same\n", encoding="utf-8")
    (tmp_path / "target-b").write_text("same\n", encoding="utf-8")
    (tmp_path / "link").symlink_to("target-a")

    first = module._manifest_for_paths(tmp_path, ["link", "missing"])
    (tmp_path / "link").unlink()
    (tmp_path / "link").symlink_to("target-b")
    second = module._manifest_for_paths(tmp_path, ["link", "missing"])
    assert first != second

    (tmp_path / "missing").write_text("now present\n", encoding="utf-8")
    assert second != module._manifest_for_paths(tmp_path, ["link", "missing"])


def test_manifest_tracks_executable_mode(tmp_path: Path) -> None:
    module = load_module()
    script = tmp_path / "gate.sh"
    script.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    script.chmod(0o644)
    regular = module._manifest_for_paths(tmp_path, ["gate.sh"])
    script.chmod(0o755)
    executable = module._manifest_for_paths(tmp_path, ["gate.sh"])
    assert regular != executable
    assert os.access(script, os.X_OK)


def test_source_seal_is_deterministic_and_round_trips_exact_closure(
    tmp_path: Path,
) -> None:
    (
        module,
        source,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    second_archive = tmp_path / "source-second.seal"
    assert (
        module.create_source_seal(
            source, path_list, second_archive, manifest
        )
        == archive_sha
    )
    assert second_archive.read_bytes() == archive.read_bytes()

    destination = tmp_path / "detached"
    destination.mkdir()
    assert (
        module.extract_source_seal(
            archive,
            path_list,
            destination,
            manifest,
            archive_sha,
            _sha256(path_list),
        )
        == manifest
    )
    assert (destination / "bin" / "runner").read_bytes() == (
        source / "bin" / "runner"
    ).read_bytes()
    assert stat.S_IMODE((destination / "bin" / "runner").stat().st_mode) == 0o751
    assert stat.S_IMODE((destination / "empty").stat().st_mode) == stat.S_IMODE(
        (source / "empty").stat().st_mode
    )
    assert (destination / "link").is_symlink()
    assert os.readlink(destination / "link") == "payload"
    assert not (destination / "deleted").exists()
    assert (
        module.workspace_source_manifest_from_path_list(destination, path_list)
        == manifest
    )


@pytest.mark.parametrize(
    "unsafe_path",
    (
        "/absolute",
        "../escape",
        "nested/../../escape",
        ".git",
        ".git/config",
        "double//separator",
        "./dot",
    ),
)
def test_source_path_list_rejects_unsafe_archive_names(
    tmp_path: Path, unsafe_path: str
) -> None:
    module = load_module()
    with pytest.raises(module.SourcePathListError, match="unsafe path"):
        module.write_source_path_list(
            tmp_path / "unsafe-source-paths.bin", [unsafe_path]
        )


def test_source_seal_rejects_out_of_root_symlinks_on_create_and_extract(
    tmp_path: Path,
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "link").symlink_to("../escape")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["link"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    with pytest.raises(module.SourceSealError, match="out-of-root symlink"):
        module.create_source_seal(
            source, path_list, tmp_path / "unsafe.seal", manifest
        )

    (source / "link").unlink()
    (source / "link").symlink_to("safe")
    safe_manifest = module.workspace_source_manifest_from_path_list(
        source, path_list
    )
    malicious = tmp_path / "malicious.seal"
    malicious.write_bytes(
        _seal_bytes(module, [(b"link", b"L", 0o777, b"../../escape")])
    )
    destination = tmp_path / "destination"
    destination.mkdir()
    with pytest.raises(module.SourceSealError, match="out-of-root symlink"):
        module.extract_source_seal(
            malicious,
            path_list,
            destination,
            safe_manifest,
            _sha256(malicious),
            _sha256(path_list),
        )
    assert list(destination.iterdir()) == []


def test_in_root_dangling_symlink_cannot_smuggle_unsealed_target(
    tmp_path: Path,
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "link").symlink_to("generated-but-unsealed")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["link"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    archive = tmp_path / "source.seal"
    archive_sha = module.create_source_seal(
        source, path_list, archive, manifest
    )
    destination = tmp_path / "destination"
    destination.mkdir()
    module.extract_source_seal(
        archive,
        path_list,
        destination,
        manifest,
        archive_sha,
        _sha256(path_list),
    )
    assert (destination / "link").is_symlink()
    assert not (destination / "link").exists()

    (destination / "generated-but-unsealed").write_bytes(b"smuggled input")
    with pytest.raises(module.SourceSealError, match="extra or hard-linked"):
        module.workspace_source_manifest_from_exact_path_list(
            destination, path_list
        )


@pytest.mark.parametrize(
    ("records", "message"),
    (
        (
            [(b"a", b"F", 0o644, b"a"), (b"a", b"F", 0o644, b"a")],
            "outside or out of order",
        ),
        (
            [(b"a", b"F", 0o644, b"a"), (b"outside", b"F", 0o644, b"b")],
            "outside or out of order",
        ),
        (
            [(b"/absolute", b"F", 0o644, b"a"), (b"b", b"F", 0o644, b"b")],
            "unsafe path",
        ),
        (
            [(b"../escape", b"F", 0o644, b"a"), (b"b", b"F", 0o644, b"b")],
            "unsafe path",
        ),
        (
            [(b".git", b"F", 0o644, b"a"), (b"b", b"F", 0o644, b"b")],
            "unsafe path",
        ),
    ),
    ids=("duplicate", "outside-closure", "absolute", "dotdot", "dot-git"),
)
def test_source_seal_rejects_duplicate_outside_and_unsafe_members_before_extract(
    tmp_path: Path,
    records: list[tuple[bytes, bytes, int, bytes]],
    message: str,
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "a").write_bytes(b"a")
    (source / "b").write_bytes(b"b")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["a", "b"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    archive = tmp_path / "malicious.seal"
    archive.write_bytes(_seal_bytes(module, records))
    destination = tmp_path / "destination"
    destination.mkdir()

    with pytest.raises(module.SourceSealError, match=message):
        module.extract_source_seal(
            archive,
            path_list,
            destination,
            manifest,
            _sha256(archive),
            _sha256(path_list),
        )
    assert list(destination.iterdir()) == []


@pytest.mark.parametrize("kind", (b"H", b"C", b"B", b"P", b"S"))
def test_source_seal_rejects_hard_link_device_fifo_and_socket_member_kinds(
    tmp_path: Path, kind: bytes
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "member").write_bytes(b"x")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["member"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    archive = tmp_path / "malicious.seal"
    archive.write_bytes(_seal_bytes(module, [(b"member", kind, 0o644, b"")]))
    destination = tmp_path / "destination"
    destination.mkdir()

    with pytest.raises(module.SourceSealError, match="hard link, device, FIFO, socket"):
        module.extract_source_seal(
            archive,
            path_list,
            destination,
            manifest,
            _sha256(archive),
            _sha256(path_list),
        )
    assert list(destination.iterdir()) == []


def test_source_seal_rejects_hard_link_fifo_and_socket_sources(
    tmp_path: Path,
) -> None:
    module = load_module()

    hard_link_root = tmp_path / "hard-link-source"
    hard_link_root.mkdir()
    (hard_link_root / "first").write_bytes(b"same inode")
    os.link(hard_link_root / "first", hard_link_root / "second")
    hard_link_paths = tmp_path / "hard-link-paths.bin"
    module.write_source_path_list(hard_link_paths, ["first", "second"])
    hard_link_manifest = module.workspace_source_manifest_from_path_list(
        hard_link_root, hard_link_paths
    )
    with pytest.raises(module.SourceSealError, match="hard-linked regular"):
        module.create_source_seal(
            hard_link_root,
            hard_link_paths,
            tmp_path / "hard-link.seal",
            hard_link_manifest,
        )

    fifo_root = tmp_path / "fifo-source"
    fifo_root.mkdir()
    os.mkfifo(fifo_root / "member")
    fifo_paths = tmp_path / "fifo-paths.bin"
    module.write_source_path_list(fifo_paths, ["member"])
    fifo_manifest = module.workspace_source_manifest_from_path_list(
        fifo_root, fifo_paths
    )
    with pytest.raises(module.SourceSealError, match="device, FIFO, socket"):
        module.create_source_seal(
            fifo_root, fifo_paths, tmp_path / "fifo.seal", fifo_manifest
        )

    with tempfile.TemporaryDirectory(prefix="iroha-seal-", dir="/tmp") as short:
        socket_root = Path(short)
        socket_path = socket_root / "member"
        with socket.socket(socket.AF_UNIX) as unix_socket:
            unix_socket.bind(str(socket_path))
            socket_paths = tmp_path / "socket-paths.bin"
            module.write_source_path_list(socket_paths, ["member"])
            socket_manifest = module.workspace_source_manifest_from_path_list(
                socket_root, socket_paths
            )
            with pytest.raises(
                module.SourceSealError, match="device, FIFO, socket"
            ):
                module.create_source_seal(
                    socket_root,
                    socket_paths,
                    tmp_path / "socket.seal",
                    socket_manifest,
                )


def test_source_seal_rejects_mutated_archive_and_path_list(
    tmp_path: Path,
) -> None:
    (
        module,
        _,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    original_path_list_sha = _sha256(path_list)

    mutated_archive = tmp_path / "mutated.seal"
    payload = bytearray(archive.read_bytes())
    payload[-1] ^= 1
    mutated_archive.write_bytes(payload)
    destination = tmp_path / "archive-destination"
    destination.mkdir()
    with pytest.raises(module.SourceSealError, match="archive SHA-256 mismatch"):
        module.extract_source_seal(
            mutated_archive,
            path_list,
            destination,
            manifest,
            archive_sha,
            original_path_list_sha,
        )
    assert list(destination.iterdir()) == []

    mutated_path_list = tmp_path / "mutated-paths.bin"
    mutated_path_list.write_bytes(path_list.read_bytes() + b"mutation")
    destination = tmp_path / "path-list-destination"
    destination.mkdir()
    with pytest.raises(module.SourceSealError, match="path-list SHA-256 mismatch"):
        module.extract_source_seal(
            archive,
            mutated_path_list,
            destination,
            manifest,
            archive_sha,
            original_path_list_sha,
        )
    assert list(destination.iterdir()) == []


def test_source_seal_rejects_missing_member_trailing_bytes_and_size_overflow(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "member").write_bytes(b"required")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["member"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)

    missing = tmp_path / "missing.seal"
    missing.write_bytes(_seal_bytes(module, [(b"member", b"D", 0, b"")]))
    destination = tmp_path / "missing-destination"
    destination.mkdir()
    with pytest.raises(module.SourceSealError, match="workspace manifest mismatch"):
        module.extract_source_seal(
            missing,
            path_list,
            destination,
            manifest,
            _sha256(missing),
            _sha256(path_list),
        )
    assert list(destination.iterdir()) == []

    trailing = tmp_path / "trailing.seal"
    trailing.write_bytes(
        _seal_bytes(module, [(b"member", b"F", 0o644, b"required")])
        + b"trailing"
    )
    with pytest.raises(module.SourceSealError, match="trailing bytes"):
        module.extract_source_seal(
            trailing,
            path_list,
            tmp_path / "missing-destination",
            manifest,
            _sha256(trailing),
            _sha256(path_list),
        )

    oversized = tmp_path / "oversized.seal"
    oversized.write_bytes(
        _seal_bytes(module, [(b"member", b"F", 0o644, b"required")])
    )
    monkeypatch.setattr(module, "_MAX_SOURCE_FILE_BYTES", 1)
    with pytest.raises(module.SourceSealError, match="file exceeds its size bound"):
        module.extract_source_seal(
            oversized,
            path_list,
            tmp_path / "missing-destination",
            manifest,
            _sha256(oversized),
            _sha256(path_list),
        )


def test_source_seal_rejects_archive_and_member_count_overflow(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (
        module,
        _,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    destination = tmp_path / "destination"
    destination.mkdir()
    monkeypatch.setattr(
        module, "_MAX_SOURCE_SEAL_BYTES", archive.stat().st_size - 1
    )
    with pytest.raises(module.SourceSealError, match="bounded"):
        module.extract_source_seal(
            archive,
            path_list,
            destination,
            manifest,
            archive_sha,
            _sha256(path_list),
        )

    module = load_module()
    count_overflow = tmp_path / "count-overflow.seal"
    count_overflow.write_bytes(
        module._SOURCE_SEAL_DOMAIN
        + struct.pack(">Q", module._MAX_PATH_COUNT + 1)
    )
    empty = tmp_path / "count-destination"
    empty.mkdir()
    with pytest.raises(module.SourceSealError, match="member count"):
        module.extract_source_seal(
            count_overflow,
            path_list,
            empty,
            manifest,
            _sha256(count_overflow),
            _sha256(path_list),
        )

    path_count_overflow = tmp_path / "path-count-overflow.bin"
    path_count_overflow.write_bytes(
        module._PATH_LIST_DOMAIN
        + struct.pack(">Q", module._MAX_PATH_COUNT + 1)
    )
    with pytest.raises(module.SourcePathListError, match="count"):
        module.read_source_path_list(path_count_overflow)


def test_source_seal_validates_every_member_before_writing_destination(
    tmp_path: Path,
) -> None:
    module = load_module()
    source = tmp_path / "source"
    source.mkdir()
    (source / "a").write_bytes(b"a")
    (source / "b").write_bytes(b"b")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["a", "b"])
    manifest = module.workspace_source_manifest_from_path_list(source, path_list)
    archive = tmp_path / "late-malicious.seal"
    archive.write_bytes(
        _seal_bytes(
            module,
            [
                (b"a", b"F", 0o644, b"a"),
                (b"b", b"H", 0o644, b""),
            ],
        )
    )
    destination = tmp_path / "destination"
    destination.mkdir()

    with pytest.raises(module.SourceSealError, match="hard link, device, FIFO, socket"):
        module.extract_source_seal(
            archive,
            path_list,
            destination,
            manifest,
            _sha256(archive),
            _sha256(path_list),
        )
    assert list(destination.iterdir()) == []


def test_source_seal_rejects_nonempty_symlink_and_hardlinked_inputs(
    tmp_path: Path,
) -> None:
    (
        module,
        _,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    path_list_sha = _sha256(path_list)

    nonempty = tmp_path / "nonempty"
    nonempty.mkdir()
    (nonempty / "injected").write_bytes(b"extra")
    with pytest.raises(module.SourceSealError, match="destination must be empty"):
        module.extract_source_seal(
            archive,
            path_list,
            nonempty,
            manifest,
            archive_sha,
            path_list_sha,
        )

    real_destination = tmp_path / "real-destination"
    real_destination.mkdir()
    symlink_destination = tmp_path / "symlink-destination"
    symlink_destination.symlink_to(real_destination, target_is_directory=True)
    with pytest.raises(module.SourceSealError, match="must be a real directory"):
        module.extract_source_seal(
            archive,
            path_list,
            symlink_destination,
            manifest,
            archive_sha,
            path_list_sha,
        )

    hardlinked_archive = tmp_path / "hardlinked.seal"
    os.link(archive, hardlinked_archive)
    empty = tmp_path / "hardlink-destination"
    empty.mkdir()
    with pytest.raises(module.SourceSealError, match="singly linked regular"):
        module.extract_source_seal(
            hardlinked_archive,
            path_list,
            empty,
            manifest,
            archive_sha,
            path_list_sha,
        )


def test_extracted_closure_audit_rejects_extra_members(tmp_path: Path) -> None:
    (
        module,
        _,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    destination = tmp_path / "detached"
    destination.mkdir()
    module.extract_source_seal(
        archive,
        path_list,
        destination,
        manifest,
        archive_sha,
        _sha256(path_list),
    )
    (destination / "injected").write_bytes(b"outside closure")
    extractor = module._DestinationExtractor.__new__(module._DestinationExtractor)
    extractor.destination = destination
    extractor.root_descriptor, extractor.root_before = module._open_root_directory(
        destination, "source seal destination"
    )
    try:
        with pytest.raises(module.SourceSealError, match="extra or hard-linked"):
            module._audit_extracted_closure(
                extractor,
                {
                    os.fsencode(path): (
                        b"D"
                        if path == "deleted"
                        else b"G"
                        if path == "empty"
                        else b"L"
                        if path == "link"
                        else b"F"
                    )
                    for path in module.read_source_path_list(path_list)
                },
            )
    finally:
        extractor.close()


def test_exact_detached_manifest_and_minimal_context_reject_extras(
    tmp_path: Path,
) -> None:
    (
        module,
        _,
        path_list,
        manifest,
        archive,
        archive_sha,
    ) = _source_seal_fixture(tmp_path)
    destination = tmp_path / "detached"
    destination.mkdir()
    module.extract_source_seal(
        archive,
        path_list,
        destination,
        manifest,
        archive_sha,
        _sha256(path_list),
    )
    assert (
        module.workspace_source_manifest_from_exact_path_list(
            destination, path_list
        )
        == manifest
    )
    (destination / "target").mkdir()
    (destination / "target" / "injected").write_bytes(b"build output")
    with pytest.raises(module.SourceSealError, match="outside the frozen closure"):
        module.workspace_source_manifest_from_exact_path_list(
            destination, path_list
        )

    context = tmp_path / "context"
    context.mkdir()
    (context / "scripts").mkdir()
    for relative in module._SEALED_CONTEXT_FILES:
        path = context / os.fsdecode(relative)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"control\n")
    module.validate_sealed_context(context)
    (context / "injected").write_bytes(b"extra")
    with pytest.raises(module.SourceSealError, match="exact minimal inventory"):
        module.validate_sealed_context(context)


def test_minimal_context_rejects_symlink_and_hardlink_controls(
    tmp_path: Path,
) -> None:
    module = load_module()

    def make_context(name: str) -> Path:
        context = tmp_path / name
        context.mkdir()
        (context / "scripts").mkdir()
        for relative in module._SEALED_CONTEXT_FILES:
            path = context / os.fsdecode(relative)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"control\n")
        return context

    symlink_context = make_context("symlink-context")
    helper = symlink_context / "scripts" / "compute_workspace_source_manifest.py"
    helper.unlink()
    helper.symlink_to("../Dockerfile")
    with pytest.raises(module.SourceSealError, match="symlink, hard link"):
        module.validate_sealed_context(symlink_context)

    hardlink_context = make_context("hardlink-context")
    control = hardlink_context / "context-control.sha256"
    hardlink = tmp_path / "context-control-hardlink"
    os.link(control, hardlink)
    with pytest.raises(module.SourceSealError, match="symlink, hard link"):
        module.validate_sealed_context(hardlink_context)


def test_workspace_manifest_binds_ignored_cargo_lock(tmp_path: Path) -> None:
    module = load_module()
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".gitignore").write_text("Cargo.lock\n", encoding="utf-8")
    (tmp_path / "tracked.txt").write_text("source\n", encoding="utf-8")
    subprocess.run(
        ["git", "add", ".gitignore", "tracked.txt"],
        cwd=tmp_path,
        check=True,
    )

    lockfile = tmp_path / "Cargo.lock"
    lockfile.write_text("version = 3\n", encoding="utf-8")
    assert "Cargo.lock" in module._git_source_paths(tmp_path)
    first = module.workspace_source_manifest(tmp_path)

    lockfile.write_text("version = 4\n", encoding="utf-8")
    assert first != module.workspace_source_manifest(tmp_path)


def test_git_unmerged_paths_are_parsed_and_deduplicated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    output = (
        b"100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tconflict.rs\0"
        b"100644 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2\tconflict.rs\0"
        b"100644 cccccccccccccccccccccccccccccccccccccccc 3\tdocs/note.md\0"
    )

    def fake_run(*_args, **_kwargs):
        return subprocess.CompletedProcess([], 0, stdout=output)

    monkeypatch.setattr(module.subprocess, "run", fake_run)
    assert module._git_unmerged_paths(tmp_path) == [
        "conflict.rs",
        "docs/note.md",
    ]


def test_workspace_manifest_rejects_unmerged_index(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    monkeypatch.setattr(
        module,
        "_git_unmerged_paths",
        lambda _root: ["conflict.rs", "docs/note.md"],
    )

    with pytest.raises(
        module.UnmergedSourceError,
        match=r"unresolved merge entries: conflict\.rs, docs/note\.md",
    ):
        module._git_source_paths(tmp_path)


@pytest.mark.parametrize(
    ("label", "git_path", "directory"),
    [
        ("merge", "MERGE_HEAD", False),
        ("cherry-pick", "CHERRY_PICK_HEAD", False),
        ("revert", "REVERT_HEAD", False),
        ("mailbox apply", "AM_HEAD", False),
        ("rebase-apply", "rebase-apply", True),
        ("rebase-merge", "rebase-merge", True),
        ("sequencer", "sequencer", True),
        ("bisect", "BISECT_START", False),
    ],
)
def test_workspace_manifest_rejects_active_git_operations(
    tmp_path: Path, label: str, git_path: str, directory: bool
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    marker = module._git_path(tmp_path, git_path)
    if label == "bisect":
        marker.symlink_to(marker.parent / "missing-bisect-state")
    elif directory:
        marker.mkdir(parents=True)
    else:
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text("active\n", encoding="utf-8")

    with pytest.raises(module.ActiveGitOperationError, match=label):
        module.workspace_source_manifest(tmp_path)


def test_active_operation_detection_is_linked_worktree_local(tmp_path: Path) -> None:
    module = load_module()
    main = tmp_path / "main"
    linked = tmp_path / "linked"
    main.mkdir()
    init_release_repo(main)
    subprocess.run(
        ["git", "worktree", "add", "--detach", str(linked), "HEAD"],
        cwd=main,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    (linked / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")

    main_marker = module._git_path(main, "MERGE_HEAD")
    main_marker.write_text("active\n", encoding="utf-8")
    assert module._active_git_operations(linked) == []
    module.workspace_source_manifest(linked)

    linked_marker = module._git_path(linked, "MERGE_HEAD")
    linked_marker.write_text("active\n", encoding="utf-8")
    with pytest.raises(module.ActiveGitOperationError, match="merge"):
        module.workspace_source_manifest(linked)


def test_workspace_manifest_rejects_resolved_but_uncommitted_merge(
    tmp_path: Path,
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    original_branch = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=tmp_path,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    subprocess.run(["git", "switch", "-qc", "merge-side"], cwd=tmp_path, check=True)
    (tmp_path / "tracked.txt").write_text("merge side\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-qam", "merge side"], cwd=tmp_path, check=True)
    subprocess.run(["git", "switch", "-q", original_branch], cwd=tmp_path, check=True)
    (tmp_path / "tracked.txt").write_text("main side\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-qam", "main side"], cwd=tmp_path, check=True)
    merge = subprocess.run(
        ["git", "merge", "merge-side"],
        cwd=tmp_path,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert merge.returncode != 0
    (tmp_path / "tracked.txt").write_text("resolved\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)
    assert module._git_unmerged_paths(tmp_path) == []

    with pytest.raises(module.ActiveGitOperationError, match="merge"):
        module.workspace_source_manifest(tmp_path)


def test_release_identity_binds_clean_head_tree_manifest_and_lock(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)

    identity = module.release_source_identity(tmp_path)
    assert identity["head_tree"] == identity["index_tree"]
    assert identity["workspace_source_manifest_sha256"] == module.workspace_source_manifest(
        tmp_path
    )
    assert len(identity["head_commit"]) == 40
    assert len(identity["cargo_lock_sha256"]) == 64

    (tmp_path / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    changed = module.release_source_identity(tmp_path)
    assert changed["head_commit"] == identity["head_commit"]
    assert changed["head_tree"] == identity["head_tree"]
    assert changed["cargo_lock_sha256"] != identity["cargo_lock_sha256"]
    assert (
        changed["workspace_source_manifest_sha256"]
        != identity["workspace_source_manifest_sha256"]
    )


def test_release_identity_rejects_staged_source(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "tracked.txt").write_text("staged\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)

    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD"):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_tracked_worktree_drift(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "tracked.txt").write_text("dirty\n", encoding="utf-8")

    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_nonignored_untracked_source(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "untracked.rs").write_text("fn injected() {}\n", encoding="utf-8")

    with pytest.raises(
        module.DirtyReleaseSourceError, match="non-ignored untracked paths"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_missing_or_symlinked_lockfile(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    lockfile = tmp_path / "Cargo.lock"
    lockfile.unlink()
    with pytest.raises(module.DirtyReleaseSourceError, match="regular workspace Cargo.lock"):
        module.release_source_identity(tmp_path)

    ignored_target = tmp_path / "target" / "lock-target"
    ignored_target.parent.mkdir()
    ignored_target.write_text("version = 3\n", encoding="utf-8")
    lockfile.symlink_to("target/lock-target")
    with pytest.raises(module.DirtyReleaseSourceError, match="regular workspace Cargo.lock"):
        module.release_source_identity(tmp_path)


def test_release_identity_detects_same_tree_head_change(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    before = module.release_source_identity(tmp_path)
    subprocess.run(
        ["git", "commit", "--allow-empty", "-qm", "same tree, different release"],
        cwd=tmp_path,
        check=True,
    )
    after = module.release_source_identity(tmp_path)

    assert after["head_commit"] != before["head_commit"]
    assert after["head_tree"] == before["head_tree"]
    assert (
        after["workspace_source_manifest_sha256"]
        == before["workspace_source_manifest_sha256"]
    )
