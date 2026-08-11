#!/usr/bin/env python3
"""Create and verify private Sumeragi v2 release-binary bundles.

The build directories are intentionally mutable Cargo caches.  This helper
copies only the four final executables into a fresh, read-only invocation
directory and publishes an exact, externally hash-anchored manifest.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import secrets
import shutil
import stat
import sys


_MANIFEST_NAME = ".sumeragi-v2-prebuilt-binaries.tsv"
_SCHEMA_VERSION = "2"
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_TRIPLE_RE = re.compile(r"[A-Za-z0-9._-]{1,128}")
_INVOCATION_RE = re.compile(r"invocation\.[A-Za-z0-9]+")
_MAX_MANIFEST_BYTES = 32 * 1024
_MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024
_MAX_TOOL_VERSION_BYTES = 64 * 1024
_READ_CHUNK_BYTES = 1024 * 1024
_BINARY_MODE = 0o500
_MANIFEST_MODE = 0o400
_DIRECTORY_MODE = 0o500
_BUILD_DIRECTORY_MODE = 0o700

_BINARIES = (
    ("irohad", "release/iroha3d", "default"),
    (
        "irohad_message_control",
        "message-control/release/iroha3d",
        "message_control",
    ),
    ("iroha", "release/iroha", "default"),
    ("kagami", "release/kagami", "default"),
)

_KEYS = (
    "schema_version",
    "source_manifest_sha256",
    "cargo_lock_sha256",
    "cargo_version_sha256",
    "rustc_version_sha256",
    "host_triple",
    "target_triple",
    "profile",
    "bundle_dir",
    "irohad_relative_path",
    "irohad_sha256",
    "irohad_size_bytes",
    "irohad_mode_octal",
    "irohad_message_control_relative_path",
    "irohad_message_control_sha256",
    "irohad_message_control_size_bytes",
    "irohad_message_control_mode_octal",
    "iroha_relative_path",
    "iroha_sha256",
    "iroha_size_bytes",
    "iroha_mode_octal",
    "kagami_relative_path",
    "kagami_sha256",
    "kagami_size_bytes",
    "kagami_mode_octal",
)


class PrebuiltBundleError(RuntimeError):
    """The release bundle does not satisfy the publication contract."""


def _normalized_absolute(path: Path, label: str, *, must_exist: bool) -> Path:
    path = Path(path)
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise PrebuiltBundleError(f"{label} must be absolute and normalized")
    try:
        resolved = path.resolve(strict=must_exist)
    except (OSError, RuntimeError) as error:
        raise PrebuiltBundleError(f"{label} is unavailable: {path}") from error
    if resolved != path:
        raise PrebuiltBundleError(f"{label} must not contain symlinked components")
    return path


def _external_root(repo_root: Path, path: Path, label: str) -> Path:
    """Authenticate one private, owner-bound real root outside source."""

    path = _normalized_absolute(path, label, must_exist=True)
    try:
        metadata = path.lstat()
        contained = os.path.commonpath((str(path), str(repo_root))) == str(repo_root)
    except (OSError, ValueError) as error:
        raise PrebuiltBundleError(f"{label} cannot be authenticated") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != _BUILD_DIRECTORY_MODE
        or contained
    ):
        raise PrebuiltBundleError(
            f"{label} must be a private owner directory outside repository source"
        )
    return path


def _external_roots(
    repo_root: Path,
    cargo_target_dir: Path,
    artifact_root: Path,
) -> tuple[Path, Path]:
    cargo_target_dir = _external_root(
        repo_root, cargo_target_dir, "Cargo target root"
    )
    artifact_root = _external_root(
        repo_root, artifact_root, "release artifact root"
    )
    if (
        cargo_target_dir == artifact_root
        or cargo_target_dir in artifact_root.parents
        or artifact_root in cargo_target_dir.parents
    ):
        raise PrebuiltBundleError(
            "Cargo target and release artifact roots must be disjoint"
        )
    return cargo_target_dir, artifact_root


def _require_digest(value: str, label: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise PrebuiltBundleError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _stable_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _require_real_directory(path: Path, mode: int, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise PrebuiltBundleError(f"{label} is unavailable: {path}") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
    ):
        raise PrebuiltBundleError(f"{label} must be a real directory")
    if stat.S_IMODE(metadata.st_mode) != mode:
        raise PrebuiltBundleError(f"{label} mode must be exactly {mode:04o}")
    return metadata


def _require_published_file(
    path: Path,
    mode: int,
    label: str,
    *,
    max_bytes: int,
    collect_bytes: bool = True,
) -> tuple[bytes | None, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PrebuiltBundleError(f"{label} is unavailable: {path}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != mode
        or before.st_size <= 0
        or before.st_size > max_bytes
    ):
        raise PrebuiltBundleError(
            f"{label} must be a non-empty, single-link regular file with mode "
            f"{mode:04o} and size at most {max_bytes}"
        )
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrebuiltBundleError(f"{label} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if _stable_identity(before) != _stable_identity(opened):
            raise PrebuiltBundleError(f"{label} changed while it was opened")
        chunks: list[bytes] | None = [] if collect_bytes else None
        total = 0
        while chunk := os.read(descriptor, _READ_CHUNK_BYTES):
            total += len(chunk)
            if total > max_bytes:
                raise PrebuiltBundleError(f"{label} exceeds its byte bound")
            if chunks is not None:
                chunks.append(chunk)
        after = os.fstat(descriptor)
        if _stable_identity(opened) != _stable_identity(after) or total != before.st_size:
            raise PrebuiltBundleError(f"{label} changed while it was read")
        return b"".join(chunks) if chunks is not None else None, after
    finally:
        os.close(descriptor)


def _hash_file(path: Path, label: str, *, max_bytes: int) -> tuple[str, int]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PrebuiltBundleError(f"{label} is unavailable: {path}") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise PrebuiltBundleError(f"{label} must be a regular non-symlink file")
    if before.st_size <= 0 or before.st_size > max_bytes:
        raise PrebuiltBundleError(f"{label} is empty or oversized")
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrebuiltBundleError(f"{label} could not be opened safely") from error
    digest = hashlib.sha256()
    total = 0
    try:
        opened = os.fstat(descriptor)
        if (
            opened.st_dev != before.st_dev
            or opened.st_ino != before.st_ino
            or not stat.S_ISREG(opened.st_mode)
        ):
            raise PrebuiltBundleError(f"{label} changed while it was opened")
        while chunk := os.read(descriptor, _READ_CHUNK_BYTES):
            total += len(chunk)
            if total > max_bytes:
                raise PrebuiltBundleError(f"{label} exceeds its byte bound")
            digest.update(chunk)
        after = os.fstat(descriptor)
        if _stable_identity(opened) != _stable_identity(after) or total != before.st_size:
            raise PrebuiltBundleError(f"{label} changed while it was hashed")
    finally:
        os.close(descriptor)
    return digest.hexdigest(), total


def _ensure_directory_tree(authority_root: Path, path: Path) -> None:
    if path != authority_root and authority_root not in path.parents:
        raise PrebuiltBundleError("release output path escaped its authenticated root")
    relative = path.relative_to(authority_root)
    current = authority_root
    for part in relative.parts:
        if part in {"", ".", ".."}:
            raise PrebuiltBundleError("release build path is not canonical")
        current /= part
        try:
            current.mkdir(mode=_BUILD_DIRECTORY_MODE)
        except FileExistsError:
            pass
        try:
            metadata = current.lstat()
            resolved = current.resolve(strict=True)
        except (OSError, RuntimeError) as error:
            raise PrebuiltBundleError(
                f"release build directory is unavailable: {current}"
            ) from error
        if (
            resolved != current
            or stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
        ):
            raise PrebuiltBundleError(
                f"release build directory must not be a symlink: {current}"
            )


def prepare_cache(
    repo_root: Path,
    source_manifest_sha256: str,
    cargo_target_dir: Path,
    default_cache: Path,
    message_control_cache: Path,
) -> None:
    """Create only the fixed mutable build-cache directories."""

    repo_root = _normalized_absolute(repo_root, "repository root", must_exist=True)
    _require_digest(source_manifest_sha256, "source manifest")
    cargo_target_dir = _external_root(
        repo_root, cargo_target_dir, "Cargo target root"
    )
    expected_root = (
        cargo_target_dir
        / "sumeragi-v2-release"
        / source_manifest_sha256
        / "program-build-cache"
    )
    expected_default = expected_root / "default"
    expected_message = expected_root / "message-control"
    if default_cache != expected_default or message_control_cache != expected_message:
        raise PrebuiltBundleError("release build caches escaped their fixed source root")
    _ensure_directory_tree(cargo_target_dir, expected_default)
    _ensure_directory_tree(cargo_target_dir, expected_message)


def _read_tool_version(path: Path, label: str) -> bytes:
    path = _normalized_absolute(path, f"{label} capture", must_exist=True)
    try:
        before = path.lstat()
    except OSError as error:
        raise PrebuiltBundleError(f"{label} capture is unavailable") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > _MAX_TOOL_VERSION_BYTES
    ):
        raise PrebuiltBundleError(
            f"{label} capture must be a bounded, single-link regular file"
        )
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrebuiltBundleError(f"{label} capture could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if _stable_identity(before) != _stable_identity(opened):
            raise PrebuiltBundleError(f"{label} capture changed while it was opened")
        chunks: list[bytes] = []
        total = 0
        while chunk := os.read(descriptor, _READ_CHUNK_BYTES):
            total += len(chunk)
            if total > _MAX_TOOL_VERSION_BYTES:
                raise PrebuiltBundleError(f"{label} capture exceeds its byte bound")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        if _stable_identity(opened) != _stable_identity(after) or total != before.st_size:
            raise PrebuiltBundleError(f"{label} capture changed while it was read")
        data = b"".join(chunks)
    finally:
        os.close(descriptor)
    if b"\0" in data or b"\r" in data or not data.endswith(b"\n"):
        raise PrebuiltBundleError(f"{label} capture is not canonical stdout")
    return data


def _copy_binary(source: Path, destination: Path, label: str) -> tuple[str, int]:
    source_digest, source_size = _hash_file(
        source, f"{label} build output", max_bytes=_MAX_BINARY_BYTES
    )
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    flags |= getattr(os, "O_NOFOLLOW", 0)
    source_flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    source_descriptor = os.open(source, source_flags)
    try:
        source_before = os.fstat(source_descriptor)
        if not stat.S_ISREG(source_before.st_mode) or source_before.st_size != source_size:
            raise PrebuiltBundleError(f"{label} build output changed before publication")
        destination_descriptor = os.open(destination, flags, _BINARY_MODE)
        try:
            digest = hashlib.sha256()
            total = 0
            while chunk := os.read(source_descriptor, _READ_CHUNK_BYTES):
                total += len(chunk)
                if total > _MAX_BINARY_BYTES:
                    raise PrebuiltBundleError(f"{label} build output exceeds its byte bound")
                view = memoryview(chunk)
                while view:
                    written = os.write(destination_descriptor, view)
                    view = view[written:]
                digest.update(chunk)
            os.fchmod(destination_descriptor, _BINARY_MODE)
            os.fsync(destination_descriptor)
        finally:
            os.close(destination_descriptor)
        source_after = os.fstat(source_descriptor)
        if (
            _stable_identity(source_before) != _stable_identity(source_after)
            or total != source_size
            or digest.hexdigest() != source_digest
        ):
            raise PrebuiltBundleError(f"{label} build output changed during publication")
    finally:
        os.close(source_descriptor)
    published_digest, published_size = _hash_file(
        destination, f"published {label}", max_bytes=_MAX_BINARY_BYTES
    )
    metadata = destination.lstat()
    if (
        published_digest != source_digest
        or published_size != source_size
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != _BINARY_MODE
    ):
        raise PrebuiltBundleError(f"published {label} failed exact readback")
    return published_digest, published_size


def _write_exclusive(path: Path, data: bytes, mode: int) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, mode)
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_DIRECTORY", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def create_bundle(
    repo_root: Path,
    source_manifest_sha256: str,
    cargo_target_dir: Path,
    artifact_root: Path,
    default_cache: Path,
    message_control_cache: Path,
    programs_root: Path,
    cargo_version_file: Path,
    rustc_version_file: Path,
) -> tuple[Path, str]:
    """Publish and verify one fresh invocation bundle."""

    repo_root = _normalized_absolute(repo_root, "repository root", must_exist=True)
    _require_digest(source_manifest_sha256, "source manifest")
    cargo_target_dir, artifact_root = _external_roots(
        repo_root, cargo_target_dir, artifact_root
    )
    expected_programs_root = (
        artifact_root / "sumeragi-v2-release" / source_manifest_sha256 / "programs"
    )
    if programs_root != expected_programs_root:
        raise PrebuiltBundleError("programs root escaped its authenticated artifact root")
    prepare_cache(
        repo_root,
        source_manifest_sha256,
        cargo_target_dir,
        default_cache,
        message_control_cache,
    )
    _ensure_directory_tree(artifact_root, programs_root)

    cargo_version = _read_tool_version(cargo_version_file, "Cargo version")
    rustc_version = _read_tool_version(rustc_version_file, "rustc version")
    host_lines = [
        line.removeprefix(b"host: ").decode("ascii", errors="strict")
        for line in rustc_version.splitlines()
        if line.startswith(b"host: ")
    ]
    if len(host_lines) != 1 or _TRIPLE_RE.fullmatch(host_lines[0]) is None:
        raise PrebuiltBundleError("rustc -vV did not report one canonical host triple")
    host_triple = host_lines[0]

    bundle: Path | None = None
    alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    for _attempt in range(128):
        candidate = programs_root / (
            "invocation." + "".join(secrets.choice(alphabet) for _ in range(6))
        )
        try:
            candidate.mkdir(mode=_BUILD_DIRECTORY_MODE)
        except FileExistsError:
            continue
        bundle = candidate
        break
    if bundle is None or _INVOCATION_RE.fullmatch(bundle.name) is None:
        raise PrebuiltBundleError("failed to allocate a canonical private invocation bundle")
    try:
        destinations: dict[str, tuple[str, int]] = {}
        for label, relative_path, cache_kind in _BINARIES:
            relative = Path(relative_path)
            destination = bundle / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            cache = default_cache if cache_kind == "default" else message_control_cache
            source_relative = (
                relative
                if cache_kind == "default"
                else Path("release") / relative.name
            )
            destinations[label] = _copy_binary(
                cache / source_relative, destination, label
            )

        cargo_lock_sha256, _ = _hash_file(
            repo_root / "Cargo.lock",
            "workspace Cargo.lock",
            max_bytes=16 * 1024 * 1024,
        )
        rows: list[tuple[str, str]] = [
            ("schema_version", _SCHEMA_VERSION),
            ("source_manifest_sha256", source_manifest_sha256),
            ("cargo_lock_sha256", cargo_lock_sha256),
            ("cargo_version_sha256", hashlib.sha256(cargo_version).hexdigest()),
            ("rustc_version_sha256", hashlib.sha256(rustc_version).hexdigest()),
            ("host_triple", host_triple),
            ("target_triple", host_triple),
            ("profile", "release"),
            ("bundle_dir", str(bundle)),
        ]
        for label, relative_path, _cache_kind in _BINARIES:
            digest, size = destinations[label]
            rows.extend(
                (
                    (f"{label}_relative_path", relative_path),
                    (f"{label}_sha256", digest),
                    (f"{label}_size_bytes", str(size)),
                    (f"{label}_mode_octal", "0500"),
                )
            )
        if tuple(key for key, _value in rows) != _KEYS:
            raise AssertionError("release bundle manifest key order drifted")
        manifest = b"".join(
            key.encode("ascii") + b"\t" + value.encode("utf-8") + b"\n"
            for key, value in rows
        )
        if len(manifest) > _MAX_MANIFEST_BYTES:
            raise PrebuiltBundleError("release bundle manifest exceeds its byte bound")
        manifest_path = bundle / _MANIFEST_NAME
        _write_exclusive(manifest_path, manifest, _MANIFEST_MODE)

        directories = sorted(
            (path for path in bundle.rglob("*") if path.is_dir()),
            key=lambda path: len(path.parts),
            reverse=True,
        )
        for directory in directories:
            os.chmod(directory, _DIRECTORY_MODE, follow_symlinks=False)
            _fsync_directory(directory)
        os.chmod(bundle, _DIRECTORY_MODE, follow_symlinks=False)
        _fsync_directory(bundle)
        _fsync_directory(programs_root)

        manifest_sha256 = hashlib.sha256(manifest).hexdigest()
        validate_bundle(
            repo_root,
            source_manifest_sha256,
            cargo_target_dir,
            artifact_root,
            bundle,
            manifest_sha256,
        )
        return bundle, manifest_sha256
    except BaseException:
        try:
            os.chmod(bundle, _BUILD_DIRECTORY_MODE, follow_symlinks=False)
            for directory in bundle.rglob("*"):
                if directory.is_dir() and not directory.is_symlink():
                    os.chmod(directory, _BUILD_DIRECTORY_MODE, follow_symlinks=False)
                elif directory.is_file() and not directory.is_symlink():
                    os.chmod(directory, 0o600, follow_symlinks=False)
            shutil.rmtree(bundle)
        except OSError:
            pass
        raise


def _parse_manifest(data: bytes) -> dict[str, str]:
    if (
        not data
        or len(data) > _MAX_MANIFEST_BYTES
        or not data.endswith(b"\n")
        or b"\r" in data
        or b"\0" in data
    ):
        raise PrebuiltBundleError("prebuilt manifest is empty, oversized, or non-canonical")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise PrebuiltBundleError("prebuilt manifest is not canonical UTF-8") from error
    lines = text.removesuffix("\n").split("\n")
    if len(lines) != len(_KEYS):
        raise PrebuiltBundleError(
            f"prebuilt manifest must contain exactly {len(_KEYS)} fields"
        )
    values: dict[str, str] = {}
    for expected_key, line in zip(_KEYS, lines):
        fields = line.split("\t")
        if (
            len(fields) != 2
            or fields[0] != expected_key
            or not fields[1]
            or expected_key in values
        ):
            raise PrebuiltBundleError(
                f"prebuilt manifest field must be exactly {expected_key}"
            )
        values[expected_key] = fields[1]
    return values


def _validate_exact_bundle_tree(bundle: Path) -> None:
    expected_files = {
        Path(_MANIFEST_NAME),
        *(Path(relative_path) for _label, relative_path, _cache_kind in _BINARIES),
    }
    expected_directories = {
        parent
        for relative in expected_files
        for parent in relative.parents
        if parent != Path(".")
    }
    expected = expected_files | expected_directories
    observed: set[Path] = set()
    pending = [bundle]
    while pending:
        directory = pending.pop()
        try:
            iterator = os.scandir(directory)
        except OSError as error:
            raise PrebuiltBundleError(
                f"release invocation bundle tree is unavailable: {directory}"
            ) from error
        with iterator:
            for entry in iterator:
                if len(observed) >= len(expected):
                    raise PrebuiltBundleError(
                        "release invocation bundle contains more entries than its "
                        "closed inventory"
                    )
                path = Path(entry.path)
                relative = path.relative_to(bundle)
                if relative in observed or relative not in expected:
                    raise PrebuiltBundleError(
                        f"release invocation bundle contains unexpected entry: {relative}"
                    )
                observed.add(relative)
                try:
                    metadata = entry.stat(follow_symlinks=False)
                except OSError as error:
                    raise PrebuiltBundleError(
                        f"release invocation bundle entry is unavailable: {relative}"
                    ) from error
                if relative in expected_directories:
                    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(
                        metadata.st_mode
                    ):
                        raise PrebuiltBundleError(
                            f"release invocation bundle directory is not real: {relative}"
                        )
                    pending.append(path)
                elif (
                    stat.S_ISLNK(metadata.st_mode)
                    or not stat.S_ISREG(metadata.st_mode)
                    or metadata.st_nlink != 1
                ):
                    raise PrebuiltBundleError(
                        f"release invocation bundle file is not private: {relative}"
                    )
    if observed != expected:
        missing = sorted(str(path) for path in expected - observed)
        raise PrebuiltBundleError(
            "release invocation bundle is missing exact entries: " + ", ".join(missing)
        )


def validate_bundle(
    repo_root: Path,
    source_manifest_sha256: str,
    cargo_target_dir: Path,
    artifact_root: Path,
    bundle: Path,
    manifest_sha256: str,
) -> None:
    """Fail unless ``bundle`` exactly matches its inherited digest."""

    repo_root = _normalized_absolute(repo_root, "repository root", must_exist=True)
    _require_digest(source_manifest_sha256, "source manifest")
    _require_digest(manifest_sha256, "prebuilt manifest")
    bundle = _normalized_absolute(bundle, "release invocation bundle", must_exist=True)
    _cargo_target_dir, artifact_root = _external_roots(
        repo_root, cargo_target_dir, artifact_root
    )
    expected_parent = (
        artifact_root
        / "sumeragi-v2-release"
        / source_manifest_sha256
        / "programs"
    )
    if bundle.parent != expected_parent or _INVOCATION_RE.fullmatch(bundle.name) is None:
        raise PrebuiltBundleError(
            "release invocation bundle must be an immediate invocation.* child "
            "of its source-bound programs directory"
        )
    _require_real_directory(bundle, _DIRECTORY_MODE, "release invocation bundle")
    _validate_exact_bundle_tree(bundle)

    manifest_path = bundle / _MANIFEST_NAME
    manifest, _metadata = _require_published_file(
        manifest_path,
        _MANIFEST_MODE,
        "release prebuilt manifest",
        max_bytes=_MAX_MANIFEST_BYTES,
    )
    assert manifest is not None
    if hashlib.sha256(manifest).hexdigest() != manifest_sha256:
        raise PrebuiltBundleError(
            "release prebuilt manifest digest does not match the inherited anchor"
        )
    values = _parse_manifest(manifest)
    if (
        values["schema_version"] != _SCHEMA_VERSION
        or values["source_manifest_sha256"] != source_manifest_sha256
        or values["profile"] != "release"
        or values["bundle_dir"] != str(bundle)
    ):
        raise PrebuiltBundleError("release prebuilt manifest base identity mismatch")
    for key in (
        "cargo_lock_sha256",
        "cargo_version_sha256",
        "rustc_version_sha256",
    ):
        _require_digest(values[key], key)
    cargo_lock_sha256, _ = _hash_file(
        repo_root / "Cargo.lock",
        "workspace Cargo.lock",
        max_bytes=16 * 1024 * 1024,
    )
    if values["cargo_lock_sha256"] != cargo_lock_sha256:
        raise PrebuiltBundleError("release prebuilt manifest Cargo.lock digest mismatch")
    for key in ("host_triple", "target_triple"):
        if _TRIPLE_RE.fullmatch(values[key]) is None:
            raise PrebuiltBundleError(f"release prebuilt manifest {key} is invalid")

    for label, relative_path, _cache_kind in _BINARIES:
        if (
            values[f"{label}_relative_path"] != relative_path
            or values[f"{label}_mode_octal"] != "0500"
        ):
            raise PrebuiltBundleError(f"release prebuilt manifest {label} layout mismatch")
        _require_digest(values[f"{label}_sha256"], f"{label} digest")
        size_text = values[f"{label}_size_bytes"]
        if (
            not size_text.isascii()
            or not size_text.isdecimal()
            or (len(size_text) > 1 and size_text.startswith("0"))
        ):
            raise PrebuiltBundleError(f"release prebuilt manifest {label} size is invalid")
        expected_size = int(size_text)
        if expected_size <= 0 or expected_size > _MAX_BINARY_BYTES:
            raise PrebuiltBundleError(f"release prebuilt manifest {label} size is out of bounds")

        binary_path = bundle / relative_path
        current = bundle
        for component in Path(relative_path).parts[:-1]:
            current /= component
            _require_real_directory(
                current, _DIRECTORY_MODE, f"{label} parent directory"
            )
        _bytes, metadata = _require_published_file(
            binary_path,
            _BINARY_MODE,
            f"release {label} binary",
            max_bytes=_MAX_BINARY_BYTES,
            collect_bytes=False,
        )
        observed_digest, observed_size = _hash_file(
            binary_path, f"release {label} binary", max_bytes=_MAX_BINARY_BYTES
        )
        if (
            metadata.st_size != expected_size
            or observed_size != expected_size
            or observed_digest != values[f"{label}_sha256"]
        ):
            raise PrebuiltBundleError(f"release {label} binary does not match its manifest")


def _path_argument(value: str) -> Path:
    return Path(value)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare = subparsers.add_parser("prepare-cache")
    prepare.add_argument("--repo-root", type=_path_argument, required=True)
    prepare.add_argument("--source-manifest", required=True)
    prepare.add_argument("--cargo-target-dir", type=_path_argument, required=True)
    prepare.add_argument("--default-cache", type=_path_argument, required=True)
    prepare.add_argument("--message-control-cache", type=_path_argument, required=True)

    create = subparsers.add_parser("create")
    create.add_argument("--repo-root", type=_path_argument, required=True)
    create.add_argument("--source-manifest", required=True)
    create.add_argument("--cargo-target-dir", type=_path_argument, required=True)
    create.add_argument("--artifact-root", type=_path_argument, required=True)
    create.add_argument("--default-cache", type=_path_argument, required=True)
    create.add_argument("--message-control-cache", type=_path_argument, required=True)
    create.add_argument("--programs-root", type=_path_argument, required=True)
    create.add_argument("--cargo-version-file", type=_path_argument, required=True)
    create.add_argument("--rustc-version-file", type=_path_argument, required=True)

    validate = subparsers.add_parser("validate")
    validate.add_argument("--repo-root", type=_path_argument, required=True)
    validate.add_argument("--source-manifest", required=True)
    validate.add_argument("--cargo-target-dir", type=_path_argument, required=True)
    validate.add_argument("--artifact-root", type=_path_argument, required=True)
    validate.add_argument("--bundle-dir", type=_path_argument, required=True)
    validate.add_argument("--manifest-sha256", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "prepare-cache":
            prepare_cache(
                arguments.repo_root,
                arguments.source_manifest,
                arguments.cargo_target_dir,
                arguments.default_cache,
                arguments.message_control_cache,
            )
        elif arguments.command == "create":
            bundle, manifest_sha256 = create_bundle(
                arguments.repo_root,
                arguments.source_manifest,
                arguments.cargo_target_dir,
                arguments.artifact_root,
                arguments.default_cache,
                arguments.message_control_cache,
                arguments.programs_root,
                arguments.cargo_version_file,
                arguments.rustc_version_file,
            )
            print(f"bundle_dir\t{bundle}")
            print(f"manifest_sha256\t{manifest_sha256}")
        elif arguments.command == "validate":
            validate_bundle(
                arguments.repo_root,
                arguments.source_manifest,
                arguments.cargo_target_dir,
                arguments.artifact_root,
                arguments.bundle_dir,
                arguments.manifest_sha256,
            )
        else:  # pragma: no cover - argparse constrains this value.
            raise AssertionError(arguments.command)
    except (OSError, UnicodeError, PrebuiltBundleError) as error:
        print(f"release prebuilt bundle rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
