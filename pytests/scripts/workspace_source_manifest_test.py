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


def init_release_repo(path: Path, *, object_format: str = "sha1") -> None:
    init = subprocess.run(
        ["git", "init", "-q", f"--object-format={object_format}"],
        cwd=path,
        check=False,
        stderr=subprocess.PIPE,
    )
    if init.returncode != 0 and object_format == "sha256":
        pytest.skip("installed Git does not support SHA-256 repositories")
    init.check_returncode()
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


def _git_mutable_state(index: Path, objects: Path) -> tuple[object, ...]:
    index_metadata = index.lstat()
    index_state = (
        index_metadata.st_dev,
        index_metadata.st_ino,
        index_metadata.st_mode,
        index_metadata.st_nlink,
        index_metadata.st_size,
        index_metadata.st_mtime_ns,
        index_metadata.st_ctime_ns,
        index.read_bytes(),
    )
    object_state = {
        path.relative_to(objects).as_posix(): path.read_bytes()
        for path in objects.rglob("*")
        if path.is_file() and not path.is_symlink()
    }
    return index_state, object_state


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


def test_source_path_list_rejects_replacement_after_read(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    path_list = tmp_path / "source-paths.bin"
    replacement = tmp_path / "replacement-source-paths.bin"
    retained = tmp_path / "retained-source-paths.bin"
    module.write_source_path_list(path_list, ["a.txt"])
    module.write_source_path_list(replacement, ["b.txt"])
    real_lstat = Path.lstat
    observations = 0

    def replace_before_path_recheck(path: Path):
        nonlocal observations
        if path == path_list:
            observations += 1
            if observations == 2:
                path_list.rename(retained)
                replacement.rename(path_list)
        return real_lstat(path)

    monkeypatch.setattr(Path, "lstat", replace_before_path_recheck)
    with pytest.raises(module.SourcePathListError, match="replaced while it was read"):
        module.read_source_path_list(path_list)


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


def test_native_artifact_manifest_normalizes_windows_checkout_materialization(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    runner = tmp_path / "runner.sh"
    runner.write_bytes(b"#!/bin/sh\nexit 0\n")
    runner.chmod(0o755)
    link = tmp_path / "source-link"
    link.symlink_to("tracked.txt")
    if os.chmod in os.supports_follow_symlinks:
        link.chmod(0o777, follow_symlinks=False)
    tree = tmp_path / "tree"
    tree.mkdir()
    (tree / "payload").write_text("nested source\n", encoding="utf-8")
    subprocess.run(
        [
            "git",
            "add",
            "-f",
            "Cargo.lock",
            "runner.sh",
            "source-link",
            "tree/payload",
        ],
        cwd=tmp_path,
        check=True,
    )
    gitlink_oid = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=tmp_path,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    subprocess.run(
        [
            "git",
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            gitlink_oid,
            "nested",
        ],
        cwd=tmp_path,
        check=True,
    )
    (tmp_path / "nested").mkdir()
    subprocess.run(
        ["git", "commit", "-qm", "portable manifest fixture"],
        cwd=tmp_path,
        check=True,
    )

    strict = module.workspace_source_manifest(tmp_path)
    subprocess.run(
        ["git", "config", "core.filemode", "false"],
        cwd=tmp_path,
        check=True,
    )
    subprocess.run(
        ["git", "config", "core.symlinks", "false"],
        cwd=tmp_path,
        check=True,
    )
    runner.chmod(0o644)
    link.unlink()
    link.write_bytes(b"tracked.txt")
    status = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=tmp_path,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout
    assert status == ""

    monkeypatch.setattr(module.os, "supports_dir_fd", frozenset())
    assert module.native_artifact_workspace_source_manifest(tmp_path) == strict

    untracked = tmp_path / "untracked.rs"
    untracked.write_text("fn injected() {}\n", encoding="utf-8")
    with pytest.raises(module.DirtyReleaseSourceError, match="stage-zero index"):
        module.native_artifact_workspace_source_manifest(tmp_path)
    untracked.unlink()

    (tmp_path / "tracked.txt").write_text("dirty\n", encoding="utf-8")
    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.native_artifact_workspace_source_manifest(tmp_path)
    (tmp_path / "tracked.txt").write_text("source\n", encoding="utf-8")
    injected = tmp_path / "nested" / "injected"
    injected.write_text("unsealed\n", encoding="utf-8")
    with pytest.raises(module.DirtyReleaseSourceError, match="gitlink must be one empty"):
        module.native_artifact_workspace_source_manifest(tmp_path)
    injected.unlink()

    retained_tree = tmp_path / ".git" / "retained-tree"
    tree.rename(retained_tree)
    tree.symlink_to(".git/retained-tree", target_is_directory=True)
    with pytest.raises(module.SourceSealError, match="symlink.*parent"):
        module._portable_clean_index_manifest_snapshot(
            tmp_path, module._git_index_entries(tmp_path)
        )
    tree.unlink()
    retained_tree.rename(tree)

    (tmp_path / "tracked.txt").write_text("staged\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)
    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD"):
        module.native_artifact_workspace_source_manifest(tmp_path)


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


def test_source_seal_rejects_parent_replacement_during_payload_read(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    root = tmp_path / "root"
    source = root / "source"
    retained = root / "retained-source"
    replacement = root / "replacement-source"
    source.mkdir(parents=True)
    replacement.mkdir()
    (source / "member").write_bytes(b"reviewed payload")
    (replacement / "member").write_bytes(b"replacement payload")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["source/member"])
    manifest = module.workspace_source_manifest_from_path_list(root, path_list)
    archive = tmp_path / "source.seal"
    real_open = module.os.open
    swapped = False

    def replace_parent_during_payload_open(path, flags, *args, **kwargs):
        nonlocal swapped
        if path == b"member" and kwargs.get("dir_fd") is not None and not swapped:
            swapped = True
            source.rename(retained)
            replacement.rename(source)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", replace_parent_during_payload_open)
    with pytest.raises(module.SourceSealError, match="source parent changed"):
        module.create_source_seal(root, path_list, archive, manifest)
    assert swapped
    assert not archive.exists()


def test_source_seal_rejects_root_replacement_during_payload_read(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    root = tmp_path / "root"
    retained = tmp_path / "retained-root"
    replacement = tmp_path / "replacement-root"
    root.mkdir()
    replacement.mkdir()
    (root / "member").write_bytes(b"reviewed payload")
    (replacement / "member").write_bytes(b"replacement payload")
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["member"])
    manifest = module.workspace_source_manifest_from_path_list(root, path_list)
    archive = tmp_path / "source.seal"
    real_open = module.os.open
    swapped = False

    def replace_root_during_payload_open(path, flags, *args, **kwargs):
        nonlocal swapped
        if path == b"member" and kwargs.get("dir_fd") is not None and not swapped:
            swapped = True
            root.rename(retained)
            replacement.rename(root)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", replace_root_during_payload_open)
    with pytest.raises(module.SourceSealError, match="source root changed"):
        module.create_source_seal(root, path_list, archive, manifest)
    assert swapped
    assert not archive.exists()


def test_source_seal_rejects_deleted_member_appearing_during_seal(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    root = tmp_path / "root"
    nested = root / "nested"
    nested.mkdir(parents=True)
    deleted = nested / "deleted"
    path_list = tmp_path / "paths.bin"
    module.write_source_path_list(path_list, ["nested/deleted"])
    manifest = module.workspace_source_manifest_from_path_list(root, path_list)
    archive = tmp_path / "source.seal"
    real_stat = module.os.stat
    observations = 0

    def appear_after_deleted_revalidation(path, *args, **kwargs):
        nonlocal observations
        if path == b"deleted" and kwargs.get("dir_fd") is not None:
            observations += 1
            try:
                return real_stat(path, *args, **kwargs)
            except FileNotFoundError:
                if observations == 2:
                    deleted.write_bytes(b"late source")
                raise
        return real_stat(path, *args, **kwargs)

    monkeypatch.setattr(module.os, "stat", appear_after_deleted_revalidation)
    with pytest.raises(module.SourceSealError, match="source changed after"):
        module.create_source_seal(root, path_list, archive, manifest)
    assert observations >= 2
    assert not archive.exists()


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


@pytest.mark.parametrize(
    ("object_format", "object_id_length"), [("sha1", 40), ("sha256", 64)]
)
def test_release_identity_binds_clean_head_tree_manifest_and_lock(
    tmp_path: Path,
    object_format: str,
    object_id_length: int,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_module()
    init_release_repo(tmp_path, object_format=object_format)
    index = module._git_path(tmp_path, "index")
    objects = module._git_path(tmp_path, "objects")
    git_state = _git_mutable_state(index, objects)
    monkeypatch.setenv("GIT_DIR", str(tmp_path / "hostile-git-dir"))
    monkeypatch.setenv("GIT_INDEX_FILE", str(tmp_path / "hostile-index"))
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv("GIT_CONFIG_KEY_0", "core.fsmonitor")
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", "true")
    monkeypatch.setenv("GIT_EXTERNAL_DIFF", str(tmp_path / "hostile-diff"))
    trace_paths = [
        tmp_path / "hostile-trace",
        tmp_path / "hostile-trace2-event",
        tmp_path / "hostile-trace-curl",
    ]
    monkeypatch.setenv("GIT_TRACE", str(trace_paths[0]))
    monkeypatch.setenv("GIT_TRACE2_EVENT", str(trace_paths[1]))
    monkeypatch.setenv("GIT_TRACE_CURL", str(trace_paths[2]))

    identity = module.release_source_identity(tmp_path)
    assert _git_mutable_state(index, objects) == git_state
    assert all(not path.exists() for path in trace_paths)
    assert identity["head_tree"] == identity["index_tree"]
    assert identity["workspace_source_manifest_sha256"] == module.workspace_source_manifest(
        tmp_path
    )
    assert len(identity["head_commit"]) == object_id_length
    assert len(identity["head_tree"]) == object_id_length
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


def test_release_identity_streams_lock_once_per_snapshot(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    real_open = module.os.open
    lock_opens = 0

    def count_lock_open(path, flags, *args, **kwargs):
        nonlocal lock_opens
        if path == b"Cargo.lock" and kwargs.get("dir_fd") is not None:
            lock_opens += 1
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", count_lock_open)
    identity = module.release_source_identity(tmp_path)
    assert identity["cargo_lock_sha256"] == _sha256(tmp_path / "Cargo.lock")
    assert lock_opens == 2


@pytest.mark.parametrize("object_format", ["sha1", "sha256"])
def test_release_identity_rejects_staged_source_without_git_mutation(
    tmp_path: Path, object_format: str
) -> None:
    module = load_module()
    init_release_repo(tmp_path, object_format=object_format)
    staged = tmp_path / "staged.rs"
    staged.write_text("fn staged_source() {}\n", encoding="utf-8")
    subprocess.run(["git", "add", staged.name], cwd=tmp_path, check=True)
    index = module._git_path(tmp_path, "index")
    objects = module._git_path(tmp_path, "objects")
    git_state = _git_mutable_state(index, objects)
    worktree_state = staged.read_bytes()

    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD"):
        module.release_source_identity(tmp_path)

    assert _git_mutable_state(index, objects) == git_state
    assert staged.read_bytes() == worktree_state


def test_workspace_manifest_rejects_regular_file_replaced_before_open(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    source = tmp_path / "source.rs"
    retained = tmp_path / "retained-source.rs"
    outside = tmp_path / "outside.rs"
    source.write_text("fn reviewed() {}\n", encoding="utf-8")
    outside.write_text("fn outside() {}\n", encoding="utf-8")
    real_open = module.os.open
    replaced = False

    def replace_before_open(path, flags, *args, **kwargs):
        nonlocal replaced
        if (
            path == os.fsencode(source.name)
            and kwargs.get("dir_fd") is not None
            and not replaced
        ):
            replaced = True
            source.rename(retained)
            source.symlink_to(outside)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", replace_before_open)
    with pytest.raises(module.SourceSealError, match="changed before it was opened"):
        module._manifest_for_paths(tmp_path, ["source.rs"])
    assert outside.read_text(encoding="utf-8") == "fn outside() {}\n"


def test_workspace_manifest_rejects_parent_directory_replacement(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    source_parent = tmp_path / "source"
    retained_parent = tmp_path / "retained-source"
    replacement_parent = tmp_path / "replacement-source"
    source_parent.mkdir()
    replacement_parent.mkdir()
    (source_parent / "member.rs").write_text(
        "fn reviewed() {}\n", encoding="utf-8"
    )
    (replacement_parent / "member.rs").write_text(
        "fn outside() {}\n", encoding="utf-8"
    )
    real_open = module.os.open
    replaced = False

    def replace_parent_before_leaf_open(path, flags, *args, **kwargs):
        nonlocal replaced
        if (
            path == b"member.rs"
            and kwargs.get("dir_fd") is not None
            and not replaced
        ):
            replaced = True
            source_parent.rename(retained_parent)
            replacement_parent.rename(source_parent)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(module.os, "open", replace_parent_before_leaf_open)
    with pytest.raises(module.SourceSealError, match="source parent changed"):
        module._manifest_for_paths(tmp_path, ["source/member.rs"])


def test_release_identity_repeats_clean_index_after_manifest_capture(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    real_snapshot = module._release_workspace_snapshot
    calls = 0

    def stage_after_second_snapshot(root: Path) -> tuple[str, str]:
        nonlocal calls
        result = real_snapshot(root)
        calls += 1
        if calls == 2:
            injected = root / "late-staged.rs"
            injected.write_text("fn late_staged() {}\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", injected.name], cwd=root, check=True
            )
        return result

    monkeypatch.setattr(
        module, "_release_workspace_snapshot", stage_after_second_snapshot
    )
    with pytest.raises(
        module.DirtyReleaseSourceError,
        match="source changed during identity capture",
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_unmerged_before_deriving_a_tree(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    monkeypatch.setattr(module, "_reject_active_git_operations", lambda _root: None)
    monkeypatch.setattr(module, "_git_unmerged_paths", lambda _root: ["conflict.rs"])
    monkeypatch.setattr(
        module,
        "_git_stdout",
        lambda *_args: pytest.fail("tree derivation reached an unmerged index"),
    )

    with pytest.raises(module.UnmergedSourceError, match="conflict.rs"):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_tracked_worktree_drift(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "tracked.txt").write_text("dirty\n", encoding="utf-8")

    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.release_source_identity(tmp_path)


def test_release_identity_never_executes_repository_clean_filter(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    marker = tmp_path / "filter-executed"
    filter_script = tmp_path / ".git" / "evil-clean-filter"
    filter_script.write_text(
        "#!/bin/sh\n: >\"$1\"\ncat\n", encoding="utf-8"
    )
    filter_script.chmod(0o700)
    info_attributes = tmp_path / ".git" / "info" / "attributes"
    info_attributes.write_text("*.txt filter=evil\n", encoding="utf-8")
    subprocess.run(
        ["git", "config", "filter.evil.clean", f"{filter_script} {marker}"],
        cwd=tmp_path,
        check=True,
    )
    subprocess.run(
        ["git", "config", "filter.evil.required", "true"],
        cwd=tmp_path,
        check=True,
    )
    (tmp_path / "tracked.txt").write_text("dirty\n", encoding="utf-8")

    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.release_source_identity(tmp_path)
    assert not marker.exists()


def test_release_identity_rejects_nonignored_untracked_source(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "untracked.rs").write_text("fn injected() {}\n", encoding="utf-8")

    with pytest.raises(
        module.DirtyReleaseSourceError, match="non-ignored untracked paths"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_ignores_local_untracked_exclusions(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    external_excludes = tmp_path / ".git" / "external-excludes"
    external_excludes.write_text("hidden-by-config.rs\n", encoding="utf-8")
    subprocess.run(
        ["git", "config", "core.excludesFile", str(external_excludes)],
        cwd=tmp_path,
        check=True,
    )
    (tmp_path / ".git" / "info" / "exclude").write_text(
        "hidden-by-info.rs\n", encoding="utf-8"
    )
    (tmp_path / "hidden-by-config.rs").write_text(
        "fn hidden_by_config() {}\n", encoding="utf-8"
    )
    (tmp_path / "hidden-by-info.rs").write_text(
        "fn hidden_by_info() {}\n", encoding="utf-8"
    )

    with pytest.raises(
        module.DirtyReleaseSourceError, match="hidden-by-config.rs"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_pins_configured_core_worktree(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    redirected = tmp_path.parent / f"{tmp_path.name}-redirected-worktree"
    redirected.mkdir()
    subprocess.run(
        ["git", "config", "core.worktree", str(redirected)],
        cwd=tmp_path,
        check=True,
    )
    (tmp_path / "root-untracked.rs").write_text(
        "fn root_untracked() {}\n", encoding="utf-8"
    )

    with pytest.raises(
        module.DirtyReleaseSourceError, match="root-untracked.rs"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_untracked_ignore_policy(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    nested = tmp_path / "nested"
    nested.mkdir()
    (nested / ".gitignore").write_text("*\n", encoding="utf-8")
    (nested / "hidden.rs").write_text("fn hidden() {}\n", encoding="utf-8")

    with pytest.raises(
        module.DirtyReleaseSourceError, match="untracked ignore policy.*nested/.gitignore"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_populated_gitlink(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    nested = tmp_path / "nested"
    nested.mkdir()
    init_release_repo(nested)
    subprocess.run(["git", "add", "nested"], cwd=tmp_path, check=True)
    subprocess.run(
        ["git", "commit", "-qm", "track nested gitlink"], cwd=tmp_path, check=True
    )

    with pytest.raises(module.DirtyReleaseSourceError, match="nested"):
        module.release_source_identity(tmp_path)


def test_release_identity_does_not_ignore_staged_gitlink_change(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    first = tmp_path / "first-nested"
    second = tmp_path / "second-nested"
    first.mkdir()
    second.mkdir()
    init_release_repo(first)
    init_release_repo(second)
    (second / "tracked.txt").write_text("second revision\n", encoding="utf-8")
    subprocess.run(
        ["git", "commit", "-qam", "second revision"], cwd=second, check=True
    )
    first_oid = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=first,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    second_oid = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=second,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    subprocess.run(
        ["git", "update-index", "--add", "--cacheinfo", "160000", first_oid, "sub"],
        cwd=tmp_path,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-qm", "track gitlink"], cwd=tmp_path, check=True
    )
    subprocess.run(
        ["git", "update-index", "--cacheinfo", "160000", second_oid, "sub"],
        cwd=tmp_path,
        check=True,
    )
    subprocess.run(
        ["git", "config", "submodule.sub.ignore", "all"],
        cwd=tmp_path,
        check=True,
    )

    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD.*sub"):
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
