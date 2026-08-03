#!/usr/bin/env python3
"""Check or externally project the three generated NoritoBridge Swift hashes."""

from __future__ import annotations

import argparse
import contextlib
from dataclasses import dataclass
import fcntl
import hashlib
import importlib.util
import os
from pathlib import Path
import stat
import sys
from typing import Iterator


LOADER_RELATIVE_PATH = Path("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
EXPECTED_KEYS = {
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64",
}
PUBLISH_LOCK_NAME = ".NoritoBridge.publish.lockfile"


class PinOwnerError(RuntimeError):
    """The generated pin projection cannot be authenticated or published."""


@dataclass(frozen=True)
class ArtifactLock:
    """One authenticated lock stabilizing an Apple artifact generation."""

    path: Path
    descriptor: int

    def assert_authenticated(self) -> None:
        try:
            descriptor_metadata = os.fstat(self.descriptor)
            path_metadata = self.path.lstat()
        except OSError as error:
            raise PinOwnerError(
                f"artifact publication lock became unavailable: {error}"
            ) from error
        if (
            not stat.S_ISREG(descriptor_metadata.st_mode)
            or not stat.S_ISREG(path_metadata.st_mode)
            or stat.S_ISLNK(path_metadata.st_mode)
            or descriptor_metadata.st_nlink != 1
            or path_metadata.st_nlink != 1
            or descriptor_metadata.st_uid != os.geteuid()
            or path_metadata.st_uid != os.geteuid()
            or (descriptor_metadata.st_dev, descriptor_metadata.st_ino)
            != (path_metadata.st_dev, path_metadata.st_ino)
        ):
            raise PinOwnerError("artifact publication lock is not authenticated")

    def assert_held(self) -> None:
        self.assert_authenticated()
        try:
            fcntl.flock(self.descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise PinOwnerError("artifact publication lock is not held") from error


def _load_module(path: Path, name: str):
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise PinOwnerError(f"unable to load owner dependency: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _sha256(contents: bytes) -> str:
    return hashlib.sha256(contents).hexdigest()


def _canonical_root(path: Path) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise PinOwnerError("--root must be an absolute canonical directory")
    resolved = path.resolve(strict=True)
    if resolved != path or not path.is_dir() or path.is_symlink():
        raise PinOwnerError("--root must be a non-symbolic canonical directory")
    return path


def _canonical_external_directory(path: Path, root: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise PinOwnerError(f"{label} must be an absolute canonical directory")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise PinOwnerError(f"unable to inspect {label}: {error}") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or not os.access(path, os.R_OK | os.W_OK | os.X_OK)
        or path == root
        or root in path.parents
    ):
        raise PinOwnerError(
            f"{label} must be a non-symbolic canonical directory outside the repository"
        )
    return path


@contextlib.contextmanager
def _artifact_lock(artifact_dir: Path) -> Iterator[ArtifactLock]:
    lock_path = artifact_dir / PUBLISH_LOCK_NAME
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    directory_flags |= getattr(os, "O_NOFOLLOW", 0)
    directory_descriptor = os.open(artifact_dir, directory_flags)
    descriptor = -1
    try:
        flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(
            lock_path.name,
            flags,
            0o600,
            dir_fd=directory_descriptor,
        )
        guard = ArtifactLock(lock_path, descriptor)
        guard.assert_authenticated()
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise PinOwnerError(
                "another process holds the artifact publication lock"
            ) from error
        except OSError as error:
            raise PinOwnerError("unable to acquire the artifact publication lock") from error
        guard.assert_held()
        yield guard
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        os.close(directory_descriptor)


def _same_loader_metadata(
    expected: os.stat_result,
    actual: os.stat_result,
) -> bool:
    return (
        expected.st_dev,
        expected.st_ino,
        expected.st_mode,
        expected.st_nlink,
        expected.st_uid,
        expected.st_size,
        expected.st_mtime_ns,
        expected.st_ctime_ns,
    ) == (
        actual.st_dev,
        actual.st_ino,
        actual.st_mode,
        actual.st_nlink,
        actual.st_uid,
        actual.st_size,
        actual.st_mtime_ns,
        actual.st_ctime_ns,
    )


def _regular_loader(path: Path) -> tuple[os.stat_result, bytes]:
    try:
        if path.resolve(strict=True) != path:
            raise PinOwnerError("Swift pin target path must not traverse symbolic links")
    except OSError as error:
        raise PinOwnerError(f"unable to resolve Swift pin target: {error}") from error
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PinOwnerError(f"unable to open Swift pin target: {error}") from error
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise PinOwnerError("Swift pin target is locked by another writer") from error
        before = os.fstat(descriptor)
        visible_before = path.lstat()
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.geteuid()
            or (before.st_dev, before.st_ino)
            != (visible_before.st_dev, visible_before.st_ino)
        ):
            raise PinOwnerError("Swift pin target is not an owned regular file")
        with os.fdopen(os.dup(descriptor), "rb") as source:
            contents = source.read()
        after = os.fstat(descriptor)
        visible_after = path.lstat()
        if (
            not _same_loader_metadata(before, after)
            or (after.st_dev, after.st_ino)
            != (visible_after.st_dev, visible_after.st_ino)
        ):
            raise PinOwnerError("Swift pin target changed while its preimage was read")
        return after, contents
    finally:
        os.close(descriptor)


def _project(root: Path, artifact_dir: Path, contents: bytes) -> bytes:
    validator = _load_module(
        root / "scripts/validate_norito_bridge_xcframework.py",
        "norito_bridge_artifact_validator_for_pin_owner",
    )
    xcframework = artifact_dir / "NoritoBridge.xcframework"
    manifest = xcframework / "NoritoBridge.artifacts.json"
    link = artifact_dir / "NoritoBridge.artifacts.json"
    try:
        payload = validator.validate(
            root=root,
            xcframework=xcframework,
            manifest_path=manifest,
            manifest_link=link,
            expected_link_target="NoritoBridge.xcframework/NoritoBridge.artifacts.json",
            swift_loader=None,
            verify_repository_provenance=True,
        )
    except (OSError, validator.ValidationError) as error:
        raise PinOwnerError(str(error)) from error
    hashes = payload["hashes"]
    if not isinstance(hashes, dict) or set(hashes) != EXPECTED_KEYS:
        raise PinOwnerError("artifact manifest does not contain exactly three slice hashes")

    seal = _load_module(
        root / "scripts/norito_bridge_source_seal.py",
        "norito_bridge_source_seal_for_pin_owner",
    )
    try:
        normalized = seal.normalize_swift_native_bridge_hash_pins(contents)
        pins = seal.swift_native_bridge_hash_pins(contents)
        projected = seal.rewrite_swift_native_bridge_hash_pins(contents, hashes)
    except RuntimeError as error:
        raise PinOwnerError(str(error)) from error
    if set(pins) != EXPECTED_KEYS:
        raise PinOwnerError("Swift loader does not expose exactly the three generated pins")
    if seal.normalize_swift_native_bridge_hash_pins(projected) != normalized:
        raise PinOwnerError("pin projection changed Swift source beyond the three digests")
    return projected


def _write_new_file(path: Path, contents: bytes, mode: int, root: Path) -> None:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise PinOwnerError("generated output path must be absolute and canonical")
    parent = _canonical_external_directory(path.parent, root, "generated output parent")
    if path.name in {"", ".", ".."}:
        raise PinOwnerError("generated output path must name one regular file")
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    directory_flags |= getattr(os, "O_NOFOLLOW", 0)
    directory_descriptor = os.open(parent, directory_flags)
    parent_opened = os.fstat(directory_descriptor)
    parent_visible = parent.lstat()
    if (
        not stat.S_ISDIR(parent_opened.st_mode)
        or parent_opened.st_uid != os.geteuid()
        or (parent_opened.st_dev, parent_opened.st_ino)
        != (parent_visible.st_dev, parent_visible.st_ino)
    ):
        os.close(directory_descriptor)
        raise PinOwnerError("generated output parent is not authenticated")
    descriptor = -1
    opened: os.stat_result | None = None
    try:
        flags = os.O_RDWR | os.O_CREAT | os.O_EXCL
        flags |= getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(
            path.name,
            flags,
            mode,
            dir_fd=directory_descriptor,
        )
        opened = os.fstat(descriptor)
        visible = os.stat(
            path.name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_uid != os.geteuid()
            or (opened.st_dev, opened.st_ino) != (visible.st_dev, visible.st_ino)
        ):
            raise PinOwnerError("generated output is not an owned regular file")
        with os.fdopen(os.dup(descriptor), "wb") as output:
            output.write(contents)
            output.flush()
            os.fsync(output.fileno())
        os.lseek(descriptor, 0, os.SEEK_SET)
        written = bytearray()
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            written.extend(chunk)
        after = os.fstat(descriptor)
        visible = os.stat(
            path.name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        parent_after = parent.lstat()
        if (
            (opened.st_dev, opened.st_ino) != (after.st_dev, after.st_ino)
            or (after.st_dev, after.st_ino) != (visible.st_dev, visible.st_ino)
            or (parent_opened.st_dev, parent_opened.st_ino)
            != (parent_after.st_dev, parent_after.st_ino)
            or bytes(written) != contents
        ):
            raise PinOwnerError("generated output changed before publication")
        os.fsync(directory_descriptor)
    except FileExistsError as error:
        raise PinOwnerError(f"generated output already exists: {path}") from error
    except BaseException:
        if descriptor >= 0 and opened is not None:
            try:
                current = os.stat(
                    path.name,
                    dir_fd=directory_descriptor,
                    follow_symlinks=False,
                )
                if (current.st_dev, current.st_ino) == (opened.st_dev, opened.st_ino):
                    os.unlink(path.name, dir_fd=directory_descriptor)
            except OSError:
                pass
        raise
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        os.close(directory_descriptor)


def _open_verified_loader(
    loader: Path,
    metadata: os.stat_result,
    preimage: bytes,
) -> int:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(loader, flags)
    except OSError as error:
        raise PinOwnerError("Swift pin target changed after preimage verification") from error
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise PinOwnerError(
                "Swift pin target changed after preimage verification"
            ) from error
        before = os.fstat(descriptor)
        visible_before = loader.lstat()
        with os.fdopen(os.dup(descriptor), "rb") as source:
            contents = source.read()
        after = os.fstat(descriptor)
        visible_after = loader.lstat()
        if (
            not _same_loader_metadata(metadata, before)
            or not _same_loader_metadata(before, after)
            or (before.st_dev, before.st_ino)
            != (visible_before.st_dev, visible_before.st_ino)
            or (after.st_dev, after.st_ino)
            != (visible_after.st_dev, visible_after.st_ino)
            or contents != preimage
        ):
            raise PinOwnerError("Swift pin target changed after preimage verification")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _assert_loader_preimage(
    loader: Path,
    metadata: os.stat_result,
    preimage: bytes,
) -> None:
    descriptor = _open_verified_loader(loader, metadata, preimage)
    os.close(descriptor)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--artifact-dir", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--check",
        action="store_true",
        help="verify the repository loader without changing it",
    )
    mode.add_argument(
        "--output",
        type=Path,
        help="exclusively create one projection outside the repository",
    )
    parser.add_argument(
        "--expected-preimage-sha256",
        help="required repository-loader preimage for --output",
    )
    arguments = parser.parse_args()
    try:
        root = _canonical_root(arguments.root)
        artifact_dir = _canonical_external_directory(
            arguments.artifact_dir,
            root,
            "--artifact-dir",
        )
        loader = root / LOADER_RELATIVE_PATH
        metadata, preimage = _regular_loader(loader)
        preimage_hash = _sha256(preimage)
        if arguments.output is not None:
            if arguments.expected_preimage_sha256 is None:
                raise PinOwnerError("--output requires --expected-preimage-sha256")
            if arguments.expected_preimage_sha256 != preimage_hash:
                raise PinOwnerError(
                    "Swift pin target preimage differs from --expected-preimage-sha256"
                )
        with _artifact_lock(artifact_dir) as artifact_lock:
            projected = _project(root, artifact_dir, preimage)
            artifact_lock.assert_held()
            if arguments.check:
                _assert_loader_preimage(loader, metadata, preimage)
                if projected != preimage:
                    raise PinOwnerError("Swift native bridge pins are stale")
            elif arguments.output is not None:
                _assert_loader_preimage(loader, metadata, preimage)
                _write_new_file(
                    arguments.output,
                    projected,
                    stat.S_IMODE(metadata.st_mode),
                    root,
                )
            artifact_lock.assert_held()
    except (OSError, UnicodeError, PinOwnerError) as error:
        print(f"[-] {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
