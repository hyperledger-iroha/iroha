#!/usr/bin/env python3
"""Validate and materialize the immutable TLAPM source-build lock."""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import shlex
import shutil
import stat
import sys
from typing import Any
from urllib.parse import urlsplit


_HEX40 = re.compile(r"[0-9a-f]{40}\Z")
_HEX64 = re.compile(r"[0-9a-f]{64}\Z")
_PACKAGE_COMPONENT = re.compile(r"[A-Za-z0-9][A-Za-z0-9+_.-]*\Z")
_RELATIVE_PATH = re.compile(
    r"[A-Za-z0-9+_.-]+(?:/[A-Za-z0-9+_.-]+)*\Z"
)
_PLATFORMS = frozenset(("arm64-darwin", "x86_64-linux-gnu"))
_BACKEND_NAMES = ("community-modules", "isabelle", "ls4", "z3")
_INSTALL_ORIGINS = frozenset(
    ("caller-archive", "github-release-asset", "immutable-source-build")
)
_MAX_LOCK_BYTES = 2 * 1024 * 1024
_MAX_METADATA_BYTES = 1024 * 1024
_ISABELLE_DERIVATION_PROJECTION = "leaf-path-content-v1"
_SOURCE_BACKEND_DERIVATIONS = (
    ("ptl-to-trp", "_build/default/translate/main.exe", "lib/tlapm/backends/bin/ptl_to_trp"),
    ("zenon", "_build/default/deps/zenon/zenon", "lib/tlapm/backends/bin/zenon"),
)


class LockError(ValueError):
    """The checked-in lock or one of its derived records is invalid."""


def _exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        raise LockError(
            f"{label} keys differ: expected {sorted(expected)!r}, got {sorted(actual)!r}"
        )


def _object_without_duplicate_keys(
    pairs: list[tuple[str, Any]],
) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise LockError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _read_stable_regular_file(path: Path, *, maximum_bytes: int) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise LockError(f"cannot inspect {path}: {error}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        raise LockError(f"{path} is not one bounded regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise LockError(f"cannot open {path} safely: {error}") from error
    try:
        opened = os.fstat(descriptor)
        data = bytearray()
        while block := os.read(descriptor, 1024 * 1024):
            data.extend(block)
            if len(data) > maximum_bytes:
                raise LockError(f"{path} exceeds {maximum_bytes} bytes")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    path_after = path.lstat()
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_mode",
        "st_uid",
        "st_gid",
        "st_nlink",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
    )
    if len(data) != opened.st_size or any(
        getattr(before, field) != getattr(opened, field)
        or getattr(opened, field) != getattr(after, field)
        or getattr(after, field) != getattr(path_after, field)
        for field in stable_fields
    ):
        raise LockError(f"{path} changed while it was read")
    return bytes(data)


def _require_string(value: Any, label: str, pattern: re.Pattern[str]) -> str:
    if not isinstance(value, str) or pattern.fullmatch(value) is None:
        raise LockError(f"{label} is malformed")
    return value


def _require_https_url(value: Any, label: str) -> str:
    if not isinstance(value, str) or any(character.isspace() for character in value):
        raise LockError(f"{label} is malformed")
    parsed = urlsplit(value)
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.username is not None
        or parsed.password is not None
        or parsed.fragment
    ):
        raise LockError(f"{label} must be an absolute credential-free HTTPS URL")
    return value


def _validate_package_group(
    value: Any, label: str, *, allow_empty: bool = False
) -> list[dict[str, str]]:
    if not isinstance(value, list):
        raise LockError(f"{label} must be a list")
    if not value and not allow_empty:
        raise LockError(f"{label} must be non-empty")
    packages: list[dict[str, str]] = []
    for index, package in enumerate(value):
        if not isinstance(package, dict):
            raise LockError(f"{label}[{index}] must be an object")
        _exact_keys(package, {"name", "version"}, f"{label}[{index}]")
        name = _require_string(
            package["name"], f"{label}[{index}].name", _PACKAGE_COMPONENT
        )
        version = _require_string(
            package["version"], f"{label}[{index}].version", _PACKAGE_COMPONENT
        )
        packages.append({"name": name, "version": version})
    if packages != sorted(packages, key=lambda package: package["name"]):
        raise LockError(f"{label} must be sorted by package name")
    names = [package["name"] for package in packages]
    if len(names) != len(set(names)):
        raise LockError(f"{label} repeats a package name")
    return packages


def load_lock(path: Path) -> tuple[dict[str, Any], bytes]:
    raw = _read_stable_regular_file(path, maximum_bytes=_MAX_LOCK_BYTES)
    try:
        value = json.loads(
            raw.decode("utf-8"), object_pairs_hook=_object_without_duplicate_keys
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LockError(f"cannot decode {path} as JSON: {error}") from error
    if not isinstance(value, dict):
        raise LockError("lock root must be an object")
    _exact_keys(
        value,
        {
            "build_packages",
            "compiler_packages",
            "opam",
            "platforms",
            "schema_version",
            "source",
        },
        "lock",
    )
    if value["schema_version"] != 1 or isinstance(value["schema_version"], bool):
        raise LockError("schema_version must equal 1")

    source = value["source"]
    if not isinstance(source, dict):
        raise LockError("source must be an object")
    _exact_keys(
        source,
        {"commit", "repository", "source_date_epoch", "tree", "version"},
        "source",
    )
    _require_string(source["commit"], "source.commit", _HEX40)
    _require_string(source["tree"], "source.tree", _HEX40)
    _require_string(source["version"], "source.version", _PACKAGE_COMPONENT)
    _require_https_url(source["repository"], "source.repository")
    if (
        not isinstance(source["source_date_epoch"], int)
        or isinstance(source["source_date_epoch"], bool)
        or source["source_date_epoch"] <= 0
    ):
        raise LockError("source.source_date_epoch must be a positive integer")

    opam = value["opam"]
    if not isinstance(opam, dict):
        raise LockError("opam must be an object")
    _exact_keys(opam, {"repository", "version"}, "opam")
    _require_string(opam["version"], "opam.version", _PACKAGE_COMPONENT)
    repository = opam["repository"]
    if not isinstance(repository, dict):
        raise LockError("opam.repository must be an object")
    _exact_keys(repository, {"commit", "repository", "tree"}, "opam.repository")
    _require_string(repository["commit"], "opam.repository.commit", _HEX40)
    _require_string(repository["tree"], "opam.repository.tree", _HEX40)
    _require_https_url(repository["repository"], "opam.repository.repository")

    compiler_packages = _validate_package_group(
        value["compiler_packages"], "compiler_packages"
    )
    build_packages = _validate_package_group(value["build_packages"], "build_packages")
    compiler_names = {package["name"] for package in compiler_packages}
    build_names = {package["name"] for package in build_packages}
    overlap = compiler_names & build_names
    if overlap:
        raise LockError(f"compiler and build package groups overlap: {sorted(overlap)!r}")

    platforms = value["platforms"]
    if not isinstance(platforms, dict) or set(platforms) != _PLATFORMS:
        raise LockError(f"platforms must equal {sorted(_PLATFORMS)!r}")
    for platform_name, platform in platforms.items():
        if not isinstance(platform, dict):
            raise LockError(f"platforms.{platform_name} must be an object")
        _exact_keys(
            platform,
            {
                "additional_packages",
                "backend_downloads",
                "opam_binary",
                "package_set_sha256",
            },
            f"platforms.{platform_name}",
        )
        additional_packages = _validate_package_group(
            platform["additional_packages"],
            f"platforms.{platform_name}.additional_packages",
            allow_empty=True,
        )
        additional_names = {package["name"] for package in additional_packages}
        platform_overlap = additional_names & (compiler_names | build_names)
        if platform_overlap:
            raise LockError(
                f"platforms.{platform_name}.additional_packages overlap the common "
                f"package closure: {sorted(platform_overlap)!r}"
            )
        declared_package_set_sha256 = _require_string(
            platform["package_set_sha256"],
            f"platforms.{platform_name}.package_set_sha256",
            _HEX64,
        )
        actual_package_set_sha256 = hashlib.sha256(
            _package_table(value, platform_name)
        ).hexdigest()
        if declared_package_set_sha256 != actual_package_set_sha256:
            raise LockError(
                f"platforms.{platform_name}.package_set_sha256 does not match "
                "the exact compiler/build package table"
            )
        opam_binary = platform["opam_binary"]
        if not isinstance(opam_binary, dict):
            raise LockError(f"platforms.{platform_name}.opam_binary must be an object")
        _exact_keys(
            opam_binary,
            {"sha256", "url"},
            f"platforms.{platform_name}.opam_binary",
        )
        _require_string(
            opam_binary["sha256"],
            f"platforms.{platform_name}.opam_binary.sha256",
            _HEX64,
        )
        _require_https_url(
            opam_binary["url"], f"platforms.{platform_name}.opam_binary.url"
        )
        downloads = platform["backend_downloads"]
        if not isinstance(downloads, list) or len(downloads) != len(_BACKEND_NAMES):
            raise LockError(
                f"platforms.{platform_name}.backend_downloads must contain "
                f"{len(_BACKEND_NAMES)} records"
            )
        names: list[str] = []
        destinations: list[str] = []
        requested_urls: list[str] = []
        for index, download in enumerate(downloads):
            label = f"platforms.{platform_name}.backend_downloads[{index}]"
            if not isinstance(download, dict):
                raise LockError(f"{label} must be an object")
            _exact_keys(
                download,
                {
                    "build_path",
                    "derivation_kind",
                    "directory_prefix",
                    "destination",
                    "download_url",
                    "locked_output_architecture",
                    "locked_output_sha256",
                    "name",
                    "package_path",
                    "progress_dot_giga",
                    "requested_url",
                    "sha256",
                    "working_suffix",
                },
                label,
            )
            name = _require_string(download["name"], f"{label}.name", _PACKAGE_COMPONENT)
            _require_string(download["sha256"], f"{label}.sha256", _HEX64)
            download_url = _require_https_url(
                download["download_url"], f"{label}.download_url"
            )
            requested_url = _require_https_url(
                download["requested_url"], f"{label}.requested_url"
            )
            for path_field in ("build_path", "destination", "package_path"):
                relative = download[path_field]
                if (
                    not isinstance(relative, str)
                    or _RELATIVE_PATH.fullmatch(relative) is None
                ):
                    raise LockError(f"{label}.{path_field} is malformed")
                relative_path = PurePosixPath(relative)
                if (
                    relative_path.is_absolute()
                    or not relative_path.parts
                    or any(part in ("", ".", "..") for part in relative_path.parts)
                    or relative_path.as_posix() != relative
                ):
                    raise LockError(
                        f"{label}.{path_field} is not a normalized relative path"
                    )
            if download["derivation_kind"] not in ("file", "tree"):
                raise LockError(f"{label}.derivation_kind must be file or tree")
            directory_prefix = download["directory_prefix"]
            if directory_prefix is not None and (
                not isinstance(directory_prefix, str)
                or _RELATIVE_PATH.fullmatch(directory_prefix) is None
            ):
                raise LockError(f"{label}.directory_prefix is malformed")
            working_suffix = download["working_suffix"]
            if (
                not isinstance(working_suffix, str)
                or _RELATIVE_PATH.fullmatch(working_suffix) is None
            ):
                raise LockError(f"{label}.working_suffix is malformed")
            locked_output_sha256 = download["locked_output_sha256"]
            if locked_output_sha256 is not None:
                _require_string(
                    locked_output_sha256, f"{label}.locked_output_sha256", _HEX64
                )
                if download["derivation_kind"] != "file":
                    raise LockError(
                        f"{label}.locked_output_sha256 is valid only for a file derivation"
                    )
            architecture = download["locked_output_architecture"]
            if architecture is not None and (
                not isinstance(architecture, str)
                or _PACKAGE_COMPONENT.fullmatch(architecture) is None
            ):
                raise LockError(f"{label}.locked_output_architecture is malformed")
            if not isinstance(download["progress_dot_giga"], bool):
                raise LockError(f"{label}.progress_dot_giga must be boolean")
            destination = download["destination"]
            if PurePosixPath(requested_url.split("?", 1)[0]).name != PurePosixPath(
                destination
            ).name:
                raise LockError(
                    f"{label}.destination basename must match the requested URL"
                )
            if name != "community-modules" and download_url != requested_url:
                raise LockError(
                    f"{label} may use a distinct immutable download URL only for "
                    "the reviewed CommunityModules latest-URL bridge"
                )
            names.append(name)
            destinations.append(destination)
            requested_urls.append(requested_url)
        if names != list(_BACKEND_NAMES):
            raise LockError(
                f"platforms.{platform_name}.backend_downloads must lock "
                f"{', '.join(_BACKEND_NAMES)} in canonical order"
            )
        if len(destinations) != len(set(destinations)):
            raise LockError(f"platforms.{platform_name} repeats a backend destination")
        if len(requested_urls) != len(set(requested_urls)):
            raise LockError(f"platforms.{platform_name} repeats a requested backend URL")
    return value, raw


def _packages(
    lock: dict[str, Any], group: str, platform: str
) -> list[dict[str, str]]:
    platform_packages = lock["platforms"][platform]["additional_packages"]
    if group == "compiler":
        selected = lock["compiler_packages"]
    elif group == "build":
        selected = lock["build_packages"] + platform_packages
    elif group == "all":
        selected = (
            lock["compiler_packages"] + lock["build_packages"] + platform_packages
        )
    else:
        raise LockError(f"unknown package group: {group}")
    return sorted(selected, key=lambda package: package["name"])


def _package_table(lock: dict[str, Any], platform: str) -> bytes:
    return "".join(
        f"{package['name']}\t{package['version']}\n"
        for package in _packages(lock, "all", platform)
    ).encode("utf-8")


def _hash_regular_file(path: Path) -> str:
    maximum = 4 * 1024 * 1024 * 1024
    try:
        before = path.lstat()
    except OSError as error:
        raise LockError(f"cannot inspect {path}: {error}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum
    ):
        raise LockError(f"{path} is not one bounded regular archive")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        digest = hashlib.sha256()
        total = 0
        while block := os.read(descriptor, 1024 * 1024):
            digest.update(block)
            total += len(block)
            if total > maximum:
                raise LockError(f"{path} exceeds {maximum} bytes")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    path_after = path.lstat()
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_mode",
        "st_uid",
        "st_gid",
        "st_nlink",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
    )
    if total != opened.st_size or any(
        getattr(before, field) != getattr(opened, field)
        or getattr(opened, field) != getattr(after, field)
        or getattr(after, field) != getattr(path_after, field)
        for field in stable_fields
    ):
        raise LockError(f"{path} changed while it was hashed")
    return digest.hexdigest()


def _write_private_file(path: Path, payload: bytes, *, mode: int = 0o400) -> None:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise LockError(f"output path is not absolute and normalized: {path}")
    if path.exists() or path.is_symlink():
        raise LockError(f"output path already exists: {path}")
    parent = path.parent
    parent_metadata = parent.lstat()
    if (
        not stat.S_ISDIR(parent_metadata.st_mode)
        or stat.S_ISLNK(parent_metadata.st_mode)
        or parent_metadata.st_uid != os.getuid()
        or parent_metadata.st_mode & 0o077
        or os.path.realpath(parent) != str(parent)
    ):
        raise LockError(f"output parent is not one canonical owner-private directory: {parent}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, mode)
    try:
        written = 0
        while written < len(payload):
            count = os.write(descriptor, payload[written:])
            if count <= 0:
                raise LockError(f"short write while publishing {path}")
            written += count
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write_private_json(path: Path, value: dict[str, Any]) -> None:
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    _write_private_file(path, payload)


def _safe_relative_symlink(root: Path, path: Path, target: str) -> None:
    if not target or "\x00" in target or os.path.isabs(target):
        raise LockError(f"tree contains an unsafe symlink: {path}")
    resolved = os.path.normpath(os.path.join(str(path.parent), target))
    try:
        inside = os.path.commonpath((str(root), resolved)) == str(root)
    except ValueError:
        inside = False
    if not inside:
        raise LockError(f"tree contains an escaping symlink: {path}")


def _tree_digest(
    root: Path, *, include_modes: bool, include_directories: bool = True
) -> str:
    if not root.is_absolute() or Path(os.path.abspath(root)) != root:
        raise LockError(f"tree root is not absolute and normalized: {root}")
    try:
        root_metadata = root.lstat()
    except OSError as error:
        raise LockError(f"cannot inspect tree root {root}: {error}") from error
    if (
        stat.S_ISLNK(root_metadata.st_mode)
        or not stat.S_ISDIR(root_metadata.st_mode)
        or os.path.realpath(root) != str(root)
    ):
        raise LockError(f"tree root is not one canonical directory: {root}")

    records: list[bytes] = []

    def visit(directory: Path) -> None:
        try:
            children = sorted(directory.iterdir(), key=lambda child: child.name)
        except OSError as error:
            raise LockError(f"cannot enumerate tree directory {directory}: {error}") from error
        for child in children:
            try:
                before = child.lstat()
            except OSError as error:
                raise LockError(f"cannot inspect tree entry {child}: {error}") from error
            relative = child.relative_to(root).as_posix()
            mode = stat.S_IMODE(before.st_mode)
            mode_field = f"\0{mode:04o}" if include_modes else ""
            if stat.S_ISDIR(before.st_mode) and not stat.S_ISLNK(before.st_mode):
                if include_directories:
                    records.append(f"D\0{relative}{mode_field}\n".encode("utf-8"))
                visit(child)
            elif stat.S_ISREG(before.st_mode) and not stat.S_ISLNK(before.st_mode):
                if before.st_nlink != 1 or before.st_mode & 0o022:
                    raise LockError(
                        f"tree file is hard-linked or group/world writable: {child}"
                    )
                file_digest = _hash_regular_file(child)
                records.append(
                    f"F\0{relative}{mode_field}\0{before.st_size}\0{file_digest}\n".encode(
                        "utf-8"
                    )
                )
            elif stat.S_ISLNK(before.st_mode):
                target = os.readlink(child)
                _safe_relative_symlink(root, child, target)
                if child.lstat() != before or os.readlink(child) != target:
                    raise LockError(f"tree symlink changed while inspected: {child}")
                records.append(
                    f"L\0{relative}{mode_field}\0{target}\n".encode("utf-8")
                )
            else:
                raise LockError(f"tree contains an unsupported entry: {child}")

    visit(root)
    digest = hashlib.sha256()
    for record in records:
        digest.update(record)
    return digest.hexdigest()


def _isabelle_executable_manifest(
    tree_root: Path,
    manifest_path: Path,
    *,
    require_exact_executable_set: bool,
) -> tuple[bytes, str]:
    if tree_root.name != "Isabelle":
        raise LockError("Isabelle derivation root has an unexpected name")
    raw = _read_stable_regular_file(
        manifest_path, maximum_bytes=_MAX_METADATA_BYTES
    )
    manifest_metadata = manifest_path.lstat()
    if manifest_metadata.st_mode & 0o022:
        raise LockError("Isabelle executable manifest is group/world writable")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise LockError("Isabelle executable manifest is not UTF-8") from error
    if not text.endswith("\n") or "\n" in text[:-1] or "\r" in text:
        raise LockError("Isabelle executable manifest is not one canonical line")
    entries = text[:-1].split(" ")
    if not entries or any(
        not entry
        or _RELATIVE_PATH.fullmatch(entry) is None
        or entry != PurePosixPath(entry).as_posix()
        or PurePosixPath(entry).parts[:1] != ("Isabelle",)
        or len(PurePosixPath(entry).parts) < 2
        or ".." in PurePosixPath(entry).parts
        for entry in entries
    ):
        raise LockError("Isabelle executable manifest contains an unsafe path")
    if len(entries) != len(set(entries)):
        raise LockError("Isabelle executable manifest contains duplicate paths")
    if "Isabelle/bin/isabelle" not in entries:
        raise LockError("Isabelle executable manifest omits its launcher")

    executable_entries: set[str] = set()

    def collect(directory: Path) -> None:
        try:
            children = sorted(directory.iterdir(), key=lambda child: child.name)
        except OSError as error:
            raise LockError(
                f"cannot enumerate Isabelle directory {directory}: {error}"
            ) from error
        for child in children:
            try:
                metadata = child.lstat()
            except OSError as error:
                raise LockError(f"cannot inspect Isabelle entry {child}: {error}") from error
            if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                collect(child)
            elif stat.S_ISREG(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                if metadata.st_mode & 0o111:
                    relative = child.relative_to(tree_root).as_posix()
                    executable_entries.add(f"Isabelle/{relative}")
            elif stat.S_ISLNK(metadata.st_mode):
                try:
                    target_metadata = child.stat()
                except OSError:
                    continue
                if stat.S_ISDIR(target_metadata.st_mode):
                    raise LockError("Isabelle tree contains a directory symlink")

    collect(tree_root)
    if require_exact_executable_set and set(entries) != executable_entries:
        raise LockError(
            "Isabelle executable manifest differs from its build executable set"
        )
    for entry in entries:
        relative = PurePosixPath(entry)
        executable_path = tree_root.joinpath(*relative.parts[1:])
        try:
            metadata = executable_path.lstat()
        except OSError as error:
            raise LockError(
                f"cannot inspect Isabelle manifest entry {executable_path}: {error}"
            ) from error
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_mode & 0o022
            or not metadata.st_mode & 0o111
        ):
            raise LockError(
                "Isabelle executable manifest does not name one safe executable file"
            )
    return raw, hashlib.sha256(raw).hexdigest()


def _distribution_closure_sha256(directory: Path) -> str:
    return _tree_digest(directory / "tlapm", include_modes=False)


def _backend_derivations(
    lock: dict[str, Any],
    platform: str,
    distribution_tree: Path,
    build_tree: Path | None,
) -> list[dict[str, Any]]:
    derivations: list[dict[str, Any]] = []
    for backend in lock["platforms"][platform]["backend_downloads"]:
        package_path = distribution_tree / "tlapm" / backend["package_path"]
        build_path = build_tree / backend["build_path"] if build_tree else None
        if backend["derivation_kind"] == "file":
            packaged_digest = _hash_regular_file(package_path)
            built_digest = (
                _hash_regular_file(build_path) if build_path is not None else packaged_digest
            )
            if backend["name"] in ("ls4", "z3"):
                for executable_path in (package_path, build_path):
                    if executable_path is None:
                        continue
                    metadata = executable_path.lstat()
                    if not stat.S_ISREG(metadata.st_mode) or not metadata.st_mode & 0o111:
                        raise LockError(
                            f"{backend['name']} derivation is not one executable file"
                        )
        else:
            packaged_digest = _tree_digest(
                package_path,
                include_modes=False,
                include_directories=False,
            )
            built_digest = (
                _tree_digest(
                    build_path,
                    include_modes=False,
                    include_directories=False,
                )
                if build_path is not None
                else packaged_digest
            )
            packaged_manifest, executable_manifest_sha256 = (
                _isabelle_executable_manifest(
                    package_path,
                    package_path.parent / "Isabelle.exec-files",
                    require_exact_executable_set=True,
                )
            )
            if build_path is not None:
                built_manifest, _ = _isabelle_executable_manifest(
                    build_path,
                    build_path.parent / "Isabelle.exec-files",
                    require_exact_executable_set=True,
                )
                if built_manifest != packaged_manifest:
                    raise LockError(
                        "packaged Isabelle executable manifest differs from its build output"
                    )
            launcher = package_path / "bin/isabelle"
            launcher_metadata = launcher.lstat()
            if (
                not stat.S_ISREG(launcher_metadata.st_mode)
                or not launcher_metadata.st_mode & 0o111
            ):
                raise LockError("packaged Isabelle launcher is not executable")
        locked_output_sha256 = backend["locked_output_sha256"]
        if locked_output_sha256 is not None and packaged_digest != locked_output_sha256:
            raise LockError(
                f"packaged {backend['name']} output differs from its locked archive member"
            )
        if built_digest != packaged_digest:
            raise LockError(
                f"packaged {backend['name']} backend is not derived from its build "
                f"output (build {built_digest}, package {packaged_digest})"
            )
        derivation = {
            "build_output_sha256": built_digest,
            "kind": backend["derivation_kind"],
            "locked_output_architecture": backend[
                "locked_output_architecture"
            ],
            "locked_output_sha256": locked_output_sha256,
            "name": backend["name"],
            "packaged_sha256": packaged_digest,
        }
        if backend["derivation_kind"] == "tree":
            derivation["executable_manifest_sha256"] = executable_manifest_sha256
            derivation["projection"] = _ISABELLE_DERIVATION_PROJECTION
        derivations.append(derivation)
    for name, build_relative, package_relative in _SOURCE_BACKEND_DERIVATIONS:
        package_path = distribution_tree / "tlapm" / package_relative
        packaged_digest = _hash_regular_file(package_path)
        built_digest = (
            _hash_regular_file(build_tree / build_relative)
            if build_tree is not None
            else packaged_digest
        )
        if built_digest != packaged_digest:
            raise LockError(f"packaged {name} backend differs from its source build output")
        for executable_path in (
            package_path,
            build_tree / build_relative if build_tree is not None else None,
        ):
            if executable_path is None:
                continue
            metadata = executable_path.lstat()
            if not stat.S_ISREG(metadata.st_mode) or not metadata.st_mode & 0o111:
                raise LockError(f"{name} backend derivation is not executable")
        derivations.append(
            {
                "build_output_sha256": built_digest,
                "kind": "file",
                "locked_output_architecture": None,
                "locked_output_sha256": None,
                "name": name,
                "packaged_sha256": packaged_digest,
            }
        )
    return derivations


def _shell_assignments(
    lock: dict[str, Any], raw: bytes, platform: str
) -> dict[str, str]:
    source = lock["source"]
    opam = lock["opam"]
    opam_repository = opam["repository"]
    opam_binary = lock["platforms"][platform]["opam_binary"]
    return {
        "TLAPM_LOCK_SHA256": hashlib.sha256(raw).hexdigest(),
        "TLAPM_OPAM_BINARY_SHA256": opam_binary["sha256"],
        "TLAPM_OPAM_BINARY_URL": opam_binary["url"],
        "TLAPM_OPAM_REPOSITORY_COMMIT": opam_repository["commit"],
        "TLAPM_OPAM_REPOSITORY_TREE": opam_repository["tree"],
        "TLAPM_OPAM_REPOSITORY_URL": opam_repository["repository"],
        "TLAPM_OPAM_VERSION": opam["version"],
        "TLAPM_PACKAGE_SET_SHA256": lock["platforms"][platform][
            "package_set_sha256"
        ],
        "TLAPM_SOURCE_COMMIT": source["commit"],
        "TLAPM_SOURCE_DATE_EPOCH": str(source["source_date_epoch"]),
        "TLAPM_SOURCE_REPOSITORY_URL": source["repository"],
        "TLAPM_SOURCE_TREE": source["tree"],
        "TLAPM_SOURCE_VERSION": source["version"],
    }


def _classify_release_fetch(curl_status: int, http_status: str) -> str:
    if curl_status < 0 or not re.fullmatch(r"[0-9]{3}", http_status):
        raise LockError("release fetch status is malformed")
    if curl_status == 0 and http_status == "200":
        return "github-release-asset"
    if curl_status == 22 and http_status in ("404", "410"):
        return "immutable-source-build"
    raise LockError(
        "exact TLAPM asset request failed without authenticated HTTP 404/410"
    )


def _expected_attestation(
    lock: dict[str, Any],
    raw: bytes,
    platform: str,
    *,
    archive_sha256: str,
    distribution_tree: Path,
    locked_wget: Path,
    source_builder: Path,
    build_tree: Path | None,
) -> dict[str, Any]:
    source = lock["source"]
    opam = lock["opam"]
    opam_repository = opam["repository"]
    platform_lock = lock["platforms"][platform]
    return {
        "archive_sha256": archive_sha256,
        "backend_derivations": _backend_derivations(
            lock, platform, distribution_tree, build_tree
        ),
        "backend_delivery": "locked-wget-v1",
        "backend_delivery_receipts_sha256": _receipt_set_sha256(lock, platform),
        "binary_identity": source["commit"][:7],
        "byte_reproducibility_claimed": False,
        "distribution_closure_sha256": _distribution_closure_sha256(
            distribution_tree
        ),
        "lock_helper_sha256": _hash_regular_file(Path(__file__).resolve(strict=True)),
        "lock_manifest_sha256": hashlib.sha256(raw).hexdigest(),
        "locked_wget_sha256": _hash_regular_file(locked_wget),
        "source_builder_sha256": _hash_regular_file(source_builder),
        "opam": {
            "binary_sha256": platform_lock["opam_binary"]["sha256"],
            "repository_commit": opam_repository["commit"],
            "repository_tree": opam_repository["tree"],
            "version": opam["version"],
        },
        "package_set_sha256": platform_lock["package_set_sha256"],
        "platform": platform,
        "pinned_backends": [
            {
                "destination": download["destination"],
                "directory_prefix": download["directory_prefix"],
                "download_url": download["download_url"],
                "locked_output_architecture": download[
                    "locked_output_architecture"
                ],
                "locked_output_sha256": download["locked_output_sha256"],
                "name": download["name"],
                "progress_dot_giga": download["progress_dot_giga"],
                "requested_url": download["requested_url"],
                "sha256": download["sha256"],
                "working_suffix": download["working_suffix"],
            }
            for download in platform_lock["backend_downloads"]
        ],
        "reproducibility_scope": "immutable upstream inputs; host build tools are outside the lock",
        "schema_version": 1,
        "source_commit": source["commit"],
        "source_tree": source["tree"],
        "source_version": source["version"],
    }


def _load_attestation(path: Path) -> dict[str, Any]:
    raw = _read_stable_regular_file(path, maximum_bytes=1024 * 1024)
    try:
        value = json.loads(
            raw.decode("utf-8"), object_pairs_hook=_object_without_duplicate_keys
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LockError(f"cannot decode {path} as JSON: {error}") from error
    if not isinstance(value, dict):
        raise LockError("source-build attestation root must be an object")
    return value


def _write_attestation(path: Path, value: dict[str, Any]) -> None:
    _write_private_json(path, value)


def _open_private_directory(path: Path, label: str) -> int:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise LockError(f"{label} is not absolute and normalized")
    try:
        before = path.lstat()
    except OSError as error:
        raise LockError(f"cannot inspect {label}: {error}") from error
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        after = path.lstat()
        fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink")
        if any(
            getattr(before, field) != getattr(opened, field)
            or getattr(opened, field) != getattr(after, field)
            for field in fields
        ):
            raise LockError(f"{label} changed while authenticated")
        if (
            stat.S_ISLNK(opened.st_mode)
            or not stat.S_ISDIR(opened.st_mode)
            or opened.st_uid != os.getuid()
            or stat.S_IMODE(opened.st_mode) != 0o700
            or os.path.realpath(path) != str(path)
        ):
            raise LockError(
                f"{label} is not one canonical owner-private mode-0700 directory"
            )
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _private_directory(path: Path, label: str) -> os.stat_result:
    descriptor = _open_private_directory(path, label)
    try:
        return os.fstat(descriptor)
    finally:
        os.close(descriptor)


def _open_private_directory_entry(
    parent_descriptor: int, name: str, label: str
) -> int:
    if not name or name in (".", "..") or "/" in name or "\x00" in name:
        raise LockError(f"{label} name is malformed")
    try:
        before = os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
    except OSError as error:
        raise LockError(f"cannot inspect {label}: {error}") from error
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent_descriptor)
    try:
        opened = os.fstat(descriptor)
        after = os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
        fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink")
        if any(
            getattr(before, field) != getattr(opened, field)
            or getattr(opened, field) != getattr(after, field)
            for field in fields
        ):
            raise LockError(f"{label} changed while descriptor-pinned")
        if (
            not stat.S_ISDIR(opened.st_mode)
            or opened.st_uid != os.getuid()
            or stat.S_IMODE(opened.st_mode) != 0o700
        ):
            raise LockError(f"{label} is not one owner-private mode-0700 directory")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _directory_entry_exists(parent_descriptor: int, name: str) -> bool:
    try:
        os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
    except FileNotFoundError:
        return False
    return True


def _write_private_file_at(
    directory_descriptor: int, name: str, payload: bytes, *, mode: int = 0o400
) -> os.stat_result:
    if not name or name in (".", "..") or "/" in name or "\x00" in name:
        raise LockError("descriptor-relative output name is malformed")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, mode, dir_fd=directory_descriptor)
    try:
        written = 0
        while written < len(payload):
            count = os.write(descriptor, payload[written:])
            if count <= 0:
                raise LockError(f"short write while publishing {name}")
            written += count
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        return os.fstat(descriptor)
    finally:
        os.close(descriptor)


def _discard_private_directory(
    parent_descriptor: int,
    directory_descriptor: int,
    directory_name: str,
    child_names: tuple[str, ...],
    label: str,
) -> None:
    for child_name in child_names:
        try:
            child = os.stat(
                child_name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            continue
        if (
            not stat.S_ISREG(child.st_mode)
            or child.st_uid != os.getuid()
            or child.st_nlink != 1
        ):
            raise LockError(f"{label} contains a replaced entry")
        os.unlink(child_name, dir_fd=directory_descriptor)
    if os.listdir(directory_descriptor):
        raise LockError(f"{label} contains unknown entries")
    pinned = os.fstat(directory_descriptor)
    named = os.stat(
        directory_name, dir_fd=parent_descriptor, follow_symlinks=False
    )
    if (pinned.st_dev, pinned.st_ino) != (named.st_dev, named.st_ino):
        raise LockError(f"{label} was replaced")
    os.rmdir(directory_name, dir_fd=parent_descriptor)


def _copy_checked_file(
    source: Path,
    destination: Path,
    *,
    expected_sha256: str | None = None,
    mode: int = 0o400,
    destination_directory_descriptor: int | None = None,
) -> str:
    try:
        before = source.lstat()
    except OSError as error:
        raise LockError(f"cannot inspect copy source {source}: {error}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > 4 * 1024 * 1024 * 1024
    ):
        raise LockError(f"copy source is not one bounded regular file: {source}")
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    destination_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(
        os, "O_CLOEXEC", 0
    )
    if hasattr(os, "O_NOFOLLOW"):
        source_flags |= os.O_NOFOLLOW
        destination_flags |= os.O_NOFOLLOW
    source_fd = os.open(source, source_flags)
    try:
        opened = os.fstat(source_fd)
        if destination_directory_descriptor is None:
            destination_fd = os.open(destination, destination_flags, mode)
        else:
            destination_name = os.fspath(destination)
            if (
                not destination_name
                or destination_name in (".", "..")
                or "/" in destination_name
                or "\x00" in destination_name
            ):
                raise LockError("descriptor-relative copy destination is malformed")
            destination_fd = os.open(
                destination_name,
                destination_flags,
                mode,
                dir_fd=destination_directory_descriptor,
            )
        try:
            digest = hashlib.sha256()
            total = 0
            while block := os.read(source_fd, 1024 * 1024):
                digest.update(block)
                total += len(block)
                if total > 4 * 1024 * 1024 * 1024:
                    raise LockError(f"copy source exceeds the archive bound: {source}")
                written = 0
                while written < len(block):
                    count = os.write(destination_fd, block[written:])
                    if count <= 0:
                        raise LockError(f"short write while copying to {destination}")
                    written += count
            os.fchmod(destination_fd, mode)
            os.fsync(destination_fd)
        finally:
            os.close(destination_fd)
        after = os.fstat(source_fd)
    finally:
        os.close(source_fd)
    path_after = source.lstat()
    fields = (
        "st_dev",
        "st_ino",
        "st_mode",
        "st_uid",
        "st_gid",
        "st_nlink",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
    )
    if total != opened.st_size or any(
        getattr(before, field) != getattr(opened, field)
        or getattr(opened, field) != getattr(after, field)
        or getattr(after, field) != getattr(path_after, field)
        for field in fields
    ):
        raise LockError(f"copy source changed while read: {source}")
    actual = digest.hexdigest()
    if expected_sha256 is not None and actual != expected_sha256:
        raise LockError(
            f"copy source checksum mismatch for {source}: expected "
            f"{expected_sha256}, got {actual}"
        )
    return actual


def _snapshot_corridor(
    raw_lock: bytes,
    helper: Path,
    locked_wget: Path,
    source_builder: Path,
    output_directory: Path,
) -> None:
    if not output_directory.is_absolute() or Path(os.path.abspath(output_directory)) != output_directory:
        raise LockError("snapshot output directory must be absolute and normalized")
    helper_bytes = _read_stable_regular_file(helper, maximum_bytes=_MAX_METADATA_BYTES)
    wget_bytes = _read_stable_regular_file(locked_wget, maximum_bytes=_MAX_METADATA_BYTES)
    builder_bytes = _read_stable_regular_file(
        source_builder, maximum_bytes=_MAX_METADATA_BYTES
    )
    parent_descriptor = _open_private_directory(
        output_directory.parent, "snapshot output parent"
    )
    directory_descriptor: int | None = None
    completed = False
    try:
        if _directory_entry_exists(parent_descriptor, output_directory.name):
            raise FileExistsError(
                errno.EEXIST, os.strerror(errno.EEXIST), output_directory
            )
        os.mkdir(output_directory.name, 0o700, dir_fd=parent_descriptor)
        directory_descriptor = _open_private_directory_entry(
            parent_descriptor,
            output_directory.name,
            "snapshot output directory",
        )
        _write_private_file_at(
            directory_descriptor, "source-build-lock.json", raw_lock
        )
        _write_private_file_at(directory_descriptor, "source-lock.py", helper_bytes)
        _write_private_file_at(
            directory_descriptor, "locked-wget.sh", wget_bytes, mode=0o500
        )
        _write_private_file_at(
            directory_descriptor, "source-builder.sh", builder_bytes, mode=0o500
        )
        os.fsync(directory_descriptor)
        os.fsync(parent_descriptor)
        completed = True
    finally:
        if directory_descriptor is not None:
            if not completed:
                _discard_private_directory(
                    parent_descriptor,
                    directory_descriptor,
                    output_directory.name,
                    (
                        "source-build-lock.json",
                        "source-lock.py",
                        "locked-wget.sh",
                        "source-builder.sh",
                    ),
                    "incomplete corridor snapshot",
                )
            os.close(directory_descriptor)
        os.close(parent_descriptor)


def _receipt_value(backend: dict[str, Any]) -> dict[str, Any]:
    return {
        "destination": backend["destination"],
        "directory_prefix": backend["directory_prefix"],
        "name": backend["name"],
        "progress_dot_giga": backend["progress_dot_giga"],
        "requested_url": backend["requested_url"],
        "schema_version": 1,
        "sha256": backend["sha256"],
        "working_suffix": backend["working_suffix"],
    }


def _receipt_set_sha256(lock: dict[str, Any], platform: str) -> str:
    values = [
        _receipt_value(backend)
        for backend in lock["platforms"][platform]["backend_downloads"]
    ]
    payload = (json.dumps(values, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )
    return hashlib.sha256(payload).hexdigest()


def _serve_wget(
    lock: dict[str, Any],
    platform: str,
    cache_directory: Path,
    output_root: Path,
    receipt_directory: Path,
    arguments: list[str],
) -> None:
    _private_directory(cache_directory, "locked download cache")
    _private_directory(receipt_directory, "locked download receipt directory")
    _private_directory(output_root, "locked wget output root")
    if arguments and arguments[0] == "--":
        arguments = arguments[1:]
    directory_prefix: str | None = None
    progress_count = 0
    urls: list[str] = []
    for argument in arguments:
        if argument == "--progress=dot:giga":
            progress_count += 1
            continue
        if argument.startswith("--directory-prefix="):
            if directory_prefix is not None:
                raise LockError("locked wget received repeated directory-prefix")
            directory_prefix = argument.split("=", 1)[1]
            continue
        if argument.startswith("-"):
            raise LockError(f"locked wget rejects unsupported argument: {argument}")
        urls.append(argument)
    if len(urls) != 1:
        raise LockError("locked wget requires exactly one reviewed URL")
    requested_url = urls[0]
    matches = [
        backend
        for backend in lock["platforms"][platform]["backend_downloads"]
        if backend["requested_url"] == requested_url
    ]
    if len(matches) != 1:
        raise LockError(f"locked wget rejects unreviewed URL: {requested_url}")
    backend = matches[0]
    expected_progress_count = 1 if backend["progress_dot_giga"] else 0
    if progress_count != expected_progress_count:
        raise LockError(
            f"locked wget progress arguments differ for {backend['name']}"
        )
    cwd = Path.cwd().resolve(strict=True)
    working_suffix = PurePosixPath(backend["working_suffix"]).parts
    if tuple(cwd.parts[-len(working_suffix) :]) != working_suffix:
        raise LockError(
            f"locked wget rejects the working directory for {backend['name']}: {cwd}"
        )
    cache_path = cache_directory / backend["destination"]
    expected_prefix = backend["directory_prefix"]
    if expected_prefix is None:
        if directory_prefix is not None:
            raise LockError(
                f"locked wget rejects a directory-prefix for {backend['name']}"
            )
        parent = cwd
    else:
        if directory_prefix is None:
            raise LockError(
                f"locked wget requires the reviewed directory-prefix for {backend['name']}"
            )
        parent = Path(directory_prefix)
        if not parent.is_absolute():
            parent = cwd / parent
    parent = Path(os.path.abspath(parent))
    try:
        inside_output = os.path.commonpath((str(output_root), str(parent))) == str(
            output_root
        )
    except ValueError:
        inside_output = False
    if not inside_output or os.path.realpath(parent) != str(parent):
        raise LockError("locked wget output escapes the authenticated build overlay")
    if expected_prefix is not None and parent != output_root / expected_prefix:
        raise LockError(
            f"locked wget directory-prefix differs for {backend['name']}: {parent}"
        )
    parent_metadata = parent.lstat()
    if (
        stat.S_ISLNK(parent_metadata.st_mode)
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != os.getuid()
        or parent_metadata.st_mode & 0o022
    ):
        raise LockError("locked wget destination parent is not owner-controlled")
    destination = parent / PurePosixPath(backend["destination"]).name
    if destination.exists() or destination.is_symlink():
        raise LockError(f"locked wget destination already exists: {destination}")
    actual = _copy_checked_file(
        cache_path,
        destination,
        expected_sha256=backend["sha256"],
    )
    if actual != backend["sha256"]:
        raise AssertionError("checked copy returned the wrong digest")
    _write_private_json(
        receipt_directory / f"{backend['name']}.json", _receipt_value(backend)
    )


def _verify_receipts(
    lock: dict[str, Any], platform: str, receipt_directory: Path
) -> None:
    _private_directory(receipt_directory, "locked download receipt directory")
    expected_names = {
        f"{backend['name']}.json"
        for backend in lock["platforms"][platform]["backend_downloads"]
    }
    actual_names = {path.name for path in receipt_directory.iterdir()}
    if actual_names != expected_names:
        raise LockError(
            f"locked wget receipt names differ: expected {sorted(expected_names)!r}, "
            f"got {sorted(actual_names)!r}"
        )
    for backend in lock["platforms"][platform]["backend_downloads"]:
        path = receipt_directory / f"{backend['name']}.json"
        raw = _read_stable_regular_file(path, maximum_bytes=_MAX_METADATA_BYTES)
        try:
            actual = json.loads(
                raw.decode("utf-8"), object_pairs_hook=_object_without_duplicate_keys
            )
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise LockError(f"cannot decode locked wget receipt {path}: {error}") from error
        if actual != _receipt_value(backend):
            raise LockError(f"locked wget receipt does not match the lock: {path}")


def _rename_no_replace_at(
    source_directory_descriptor: int,
    source_name: str,
    destination_directory_descriptor: int,
    destination_name: str,
    destination_label: Path,
) -> None:
    for name in (source_name, destination_name):
        if not name or name in (".", "..") or "/" in name or "\x00" in name:
            raise LockError("descriptor-relative publication name is malformed")
    libc = ctypes.CDLL(None, use_errno=True)
    source_bytes = os.fsencode(source_name)
    destination_bytes = os.fsencode(destination_name)
    if sys.platform == "darwin" and hasattr(libc, "renameatx_np"):
        function = libc.renameatx_np
        function.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        function.restype = ctypes.c_int
        result = function(
            source_directory_descriptor,
            source_bytes,
            destination_directory_descriptor,
            destination_bytes,
            0x00000004,
        )
    elif hasattr(libc, "renameat2"):
        function = libc.renameat2
        function.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        function.restype = ctypes.c_int
        result = function(
            source_directory_descriptor,
            source_bytes,
            destination_directory_descriptor,
            destination_bytes,
            1,
        )
    else:
        raise LockError("atomic no-replace publication is unsupported on this host")
    if result != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST:
            raise FileExistsError(error, os.strerror(error), destination_label)
        raise OSError(error, os.strerror(error), destination_label)


def _publish_output_bundle(
    archive: Path,
    attestation: Path,
    lock_raw: bytes,
    output_bundle: Path,
) -> None:
    if not output_bundle.is_absolute() or Path(os.path.abspath(output_bundle)) != output_bundle:
        raise LockError("output bundle must be absolute and normalized")
    parent = output_bundle.parent
    parent_descriptor = _open_private_directory(parent, "output bundle parent")
    stage_name = f".{output_bundle.name}.{secrets.token_hex(16)}.stage"
    stage_descriptor: int | None = None
    published = False
    try:
        if _directory_entry_exists(parent_descriptor, output_bundle.name):
            raise FileExistsError(
                errno.EEXIST, os.strerror(errno.EEXIST), output_bundle
            )
        os.mkdir(stage_name, 0o700, dir_fd=parent_descriptor)
        stage_descriptor = _open_private_directory_entry(
            parent_descriptor, stage_name, "output bundle staging directory"
        )
        _copy_checked_file(
            archive,
            Path("archive.tar.gz"),
            destination_directory_descriptor=stage_descriptor,
        )
        _copy_checked_file(
            attestation,
            Path("attestation.json"),
            destination_directory_descriptor=stage_descriptor,
        )
        _write_private_file_at(
            stage_descriptor, "source-build-lock.json", lock_raw
        )
        os.fsync(stage_descriptor)
        pinned_stage = os.fstat(stage_descriptor)
        named_stage = os.stat(
            stage_name, dir_fd=parent_descriptor, follow_symlinks=False
        )
        if (pinned_stage.st_dev, pinned_stage.st_ino) != (
            named_stage.st_dev,
            named_stage.st_ino,
        ):
            raise LockError("output bundle staging directory was replaced")
        _rename_no_replace_at(
            parent_descriptor,
            stage_name,
            parent_descriptor,
            output_bundle.name,
            output_bundle,
        )
        published = True
        os.fsync(parent_descriptor)
    finally:
        if stage_descriptor is not None:
            if not published:
                _discard_private_directory(
                    parent_descriptor,
                    stage_descriptor,
                    stage_name,
                    (
                        "archive.tar.gz",
                        "attestation.json",
                        "source-build-lock.json",
                    ),
                    "unpublished output stage",
                )
            os.close(stage_descriptor)
        os.close(parent_descriptor)


def _install_state(
    lock: dict[str, Any],
    raw: bytes,
    platform: str,
    directory: Path,
    origin: str,
    archive_sha256: str,
    attestation: Path | None,
    locked_wget: Path,
    source_builder: Path,
) -> dict[str, Any]:
    if origin not in _INSTALL_ORIGINS:
        raise LockError(f"unknown TLAPM install origin: {origin}")
    distribution_sha256 = _distribution_closure_sha256(directory)
    source_build: dict[str, str] | None = None
    if origin == "immutable-source-build":
        if attestation is None:
            raise LockError("source-built install requires an attestation")
        copied_lock = directory / "source-build-lock.json"
        copied_lock_value, copied_raw = load_lock(copied_lock)
        if copied_raw != raw or copied_lock_value != lock:
            raise LockError("installed source-build lock differs from the active lock")
        actual_attestation = _load_attestation(attestation)
        expected_attestation = _expected_attestation(
            lock,
            raw,
            platform,
            archive_sha256=archive_sha256,
            distribution_tree=directory,
            locked_wget=locked_wget,
            source_builder=source_builder,
            build_tree=None,
        )
        if actual_attestation != expected_attestation:
            raise LockError("source-build attestation does not match the installed closure")
        source_build = {
            "attestation_sha256": _hash_regular_file(attestation),
            "lock_helper_sha256": expected_attestation["lock_helper_sha256"],
            "lock_manifest_sha256": hashlib.sha256(raw).hexdigest(),
            "locked_wget_sha256": expected_attestation["locked_wget_sha256"],
            "source_builder_sha256": expected_attestation[
                "source_builder_sha256"
            ],
        }
    elif attestation is not None:
        raise LockError("prebuilt install may not carry a source-build attestation")
    return {
        "archive_sha256": archive_sha256,
        "corridor": {
            "lock_helper_sha256": _hash_regular_file(
                Path(__file__).resolve(strict=True)
            ),
            "lock_manifest_sha256": hashlib.sha256(raw).hexdigest(),
            "locked_wget_sha256": _hash_regular_file(locked_wget),
            "source_builder_sha256": _hash_regular_file(source_builder),
        },
        "distribution_closure_sha256": distribution_sha256,
        "origin": origin,
        "platform": platform,
        "schema_version": 1,
        "source_build": source_build,
        "source_commit": lock["source"]["commit"],
    }


def _read_exact_text(path: Path, label: str) -> str:
    raw = _read_stable_regular_file(path, maximum_bytes=4096)
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise LockError(f"{label} is not UTF-8") from error


def _verify_install(
    lock: dict[str, Any],
    raw: bytes,
    platform: str,
    directory: Path,
    allowed_origins: set[str],
    prebuilt_sha256: str,
    locked_wget: Path,
    source_builder: Path,
) -> str:
    _private_directory(directory, "TLAPM install directory")
    state_path = directory / "install-state.json"
    state_raw = _read_stable_regular_file(state_path, maximum_bytes=_MAX_METADATA_BYTES)
    try:
        state = json.loads(
            state_raw.decode("utf-8"), object_pairs_hook=_object_without_duplicate_keys
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LockError(f"cannot decode TLAPM install state: {error}") from error
    if not isinstance(state, dict):
        raise LockError("TLAPM install state must be an object")
    origin = state.get("origin")
    if origin not in allowed_origins:
        raise LockError(f"TLAPM install origin is not allowed for this invocation: {origin}")
    archive_sha256 = state.get("archive_sha256")
    if not isinstance(archive_sha256, str) or _HEX64.fullmatch(archive_sha256) is None:
        raise LockError("TLAPM install archive digest is malformed")
    if origin != "immutable-source-build" and archive_sha256 != prebuilt_sha256:
        raise LockError("TLAPM prebuilt install archive digest is stale")
    attestation = (
        directory / "source-build-attestation.json"
        if origin == "immutable-source-build"
        else None
    )
    expected = _install_state(
        lock,
        raw,
        platform,
        directory,
        origin,
        archive_sha256,
        attestation,
        locked_wget,
        source_builder,
    )
    if state != expected:
        raise LockError("TLAPM install state does not match its complete closure")
    if _read_exact_text(directory / "archive.sha256", "archive digest") != f"{archive_sha256}\n":
        raise LockError("TLAPM archive digest marker differs from install state")
    if _read_exact_text(directory / "archive.origin", "archive origin") != f"{origin}\n":
        raise LockError("TLAPM archive origin marker differs from install state")
    expected_entries = {
        "archive.origin",
        "archive.sha256",
        "install-state.json",
        "tlapm",
    }
    if origin == "immutable-source-build":
        expected_entries.update(("source-build-attestation.json", "source-build-lock.json"))
    actual_entries = {entry.name for entry in directory.iterdir()}
    if actual_entries != expected_entries:
        raise LockError(
            f"TLAPM install closure entries differ: expected {sorted(expected_entries)!r}, "
            f"got {sorted(actual_entries)!r}"
        )
    return origin


def _publish_directory(staged: Path, destination: Path) -> None:
    if (
        not staged.is_absolute()
        or Path(os.path.abspath(staged)) != staged
        or not destination.is_absolute()
        or Path(os.path.abspath(destination)) != destination
    ):
        raise LockError("install publication paths must be absolute and normalized")
    source_parent_descriptor = _open_private_directory(
        staged.parent, "staged TLAPM install parent"
    )
    destination_parent_descriptor = _open_private_directory(
        destination.parent, "TLAPM install parent"
    )
    staged_descriptor: int | None = None
    try:
        staged_descriptor = _open_private_directory_entry(
            source_parent_descriptor, staged.name, "staged TLAPM install"
        )
        if _directory_entry_exists(destination_parent_descriptor, destination.name):
            raise FileExistsError(
                errno.EEXIST, os.strerror(errno.EEXIST), destination
            )
        pinned_staged = os.fstat(staged_descriptor)
        named_staged = os.stat(
            staged.name,
            dir_fd=source_parent_descriptor,
            follow_symlinks=False,
        )
        if (pinned_staged.st_dev, pinned_staged.st_ino) != (
            named_staged.st_dev,
            named_staged.st_ino,
        ):
            raise LockError("staged TLAPM install was replaced before publication")
        _rename_no_replace_at(
            source_parent_descriptor,
            staged.name,
            destination_parent_descriptor,
            destination.name,
            destination,
        )
        os.fsync(destination_parent_descriptor)
        if source_parent_descriptor != destination_parent_descriptor:
            os.fsync(source_parent_descriptor)
    finally:
        if staged_descriptor is not None:
            os.close(staged_descriptor)
        os.close(destination_parent_descriptor)
        os.close(source_parent_descriptor)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", required=True, type=Path)
    parser.add_argument("--platform", required=True, choices=sorted(_PLATFORMS))
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate")
    subparsers.add_parser("emit-shell")

    classify_fetch = subparsers.add_parser("classify-release-fetch")
    classify_fetch.add_argument("--curl-status", required=True, type=int)
    classify_fetch.add_argument("--http-status", required=True)

    packages = subparsers.add_parser("emit-packages")
    packages.add_argument("--group", choices=("all", "build", "compiler"), required=True)
    packages.add_argument("--format", choices=("atom", "name", "table"), required=True)

    subparsers.add_parser("emit-backends")

    hash_file = subparsers.add_parser("hash-file")
    hash_file.add_argument("--path", required=True, type=Path)

    copy_file = subparsers.add_parser("copy-checked-file")
    copy_file.add_argument("--source", required=True, type=Path)
    copy_file.add_argument("--destination", required=True, type=Path)
    copy_file.add_argument("--expected-sha256")

    validate_directory = subparsers.add_parser("validate-private-directory")
    validate_directory.add_argument("--directory", required=True, type=Path)

    snapshot = subparsers.add_parser("snapshot-corridor")
    snapshot.add_argument("--helper", required=True, type=Path)
    snapshot.add_argument("--locked-wget", required=True, type=Path)
    snapshot.add_argument("--source-builder", required=True, type=Path)
    snapshot.add_argument("--output-dir", required=True, type=Path)

    serve_wget = subparsers.add_parser("serve-wget")
    serve_wget.add_argument("--cache-dir", required=True, type=Path)
    serve_wget.add_argument("--output-root", required=True, type=Path)
    serve_wget.add_argument("--receipt-dir", required=True, type=Path)
    serve_wget.add_argument("wget_args", nargs=argparse.REMAINDER)

    verify_receipts = subparsers.add_parser("verify-wget-receipts")
    verify_receipts.add_argument("--receipt-dir", required=True, type=Path)

    write = subparsers.add_parser("write-attestation")
    write.add_argument("--archive", required=True, type=Path)
    write.add_argument("--build-tree", required=True, type=Path)
    write.add_argument("--distribution-tree", required=True, type=Path)
    write.add_argument("--locked-wget", required=True, type=Path)
    write.add_argument("--source-builder", required=True, type=Path)
    write.add_argument("--output", required=True, type=Path)

    verify = subparsers.add_parser("verify-attestation")
    archive_input = verify.add_mutually_exclusive_group(required=True)
    archive_input.add_argument("--archive", type=Path)
    archive_input.add_argument("--archive-sha256")
    verify.add_argument("--attestation", required=True, type=Path)
    verify.add_argument("--distribution-tree", required=True, type=Path)
    verify.add_argument("--locked-wget", required=True, type=Path)
    verify.add_argument("--source-builder", required=True, type=Path)

    publish_bundle = subparsers.add_parser("publish-output-bundle")
    publish_bundle.add_argument("--archive", required=True, type=Path)
    publish_bundle.add_argument("--attestation", required=True, type=Path)
    publish_bundle.add_argument("--output-bundle", required=True, type=Path)

    write_state = subparsers.add_parser("write-install-state")
    write_state.add_argument("--directory", required=True, type=Path)
    write_state.add_argument("--origin", required=True, choices=sorted(_INSTALL_ORIGINS))
    write_state.add_argument("--archive-sha256", required=True)
    write_state.add_argument("--attestation", type=Path)
    write_state.add_argument("--locked-wget", required=True, type=Path)
    write_state.add_argument("--source-builder", required=True, type=Path)
    write_state.add_argument("--output", required=True, type=Path)

    verify_install = subparsers.add_parser("verify-install")
    verify_install.add_argument("--directory", required=True, type=Path)
    verify_install.add_argument(
        "--allowed-origin",
        action="append",
        required=True,
        choices=sorted(_INSTALL_ORIGINS),
    )
    verify_install.add_argument("--prebuilt-sha256", required=True)
    verify_install.add_argument("--locked-wget", required=True, type=Path)
    verify_install.add_argument("--source-builder", required=True, type=Path)

    publish_install = subparsers.add_parser("publish-install")
    publish_install.add_argument("--staged", required=True, type=Path)
    publish_install.add_argument("--destination", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        lock, raw = load_lock(args.lock)
        platform = args.platform
        if args.command == "validate":
            return 0
        if args.command == "emit-shell":
            for name, value in sorted(_shell_assignments(lock, raw, platform).items()):
                print(f"{name}={shlex.quote(value)}")
            return 0
        if args.command == "classify-release-fetch":
            print(_classify_release_fetch(args.curl_status, args.http_status))
            return 0
        if args.command == "emit-packages":
            for package in _packages(lock, args.group, platform):
                if args.format == "atom":
                    print(f"{package['name']}.{package['version']}")
                elif args.format == "name":
                    print(package["name"])
                else:
                    print(f"{package['name']}\t{package['version']}")
            return 0
        if args.command == "emit-backends":
            for download in lock["platforms"][platform]["backend_downloads"]:
                print(
                    "\t".join(
                        (
                            download["name"],
                            download["download_url"],
                            download["sha256"],
                            download["destination"],
                            download["locked_output_sha256"] or "-",
                            download["locked_output_architecture"] or "-",
                        )
                    )
                )
            return 0
        if args.command == "hash-file":
            print(_hash_regular_file(args.path))
            return 0
        if args.command == "copy-checked-file":
            expected_sha256 = None
            if args.expected_sha256 is not None:
                expected_sha256 = _require_string(
                    args.expected_sha256, "copy expected SHA-256", _HEX64
                )
            print(
                _copy_checked_file(
                    args.source,
                    args.destination,
                    expected_sha256=expected_sha256,
                )
            )
            return 0
        if args.command == "validate-private-directory":
            _private_directory(args.directory, "requested private directory")
            return 0
        if args.command == "snapshot-corridor":
            _snapshot_corridor(
                raw,
                args.helper,
                args.locked_wget,
                args.source_builder,
                args.output_dir,
            )
            return 0
        if args.command == "serve-wget":
            _serve_wget(
                lock,
                platform,
                args.cache_dir,
                args.output_root,
                args.receipt_dir,
                args.wget_args,
            )
            return 0
        if args.command == "verify-wget-receipts":
            _verify_receipts(lock, platform, args.receipt_dir)
            return 0
        if args.command == "write-attestation":
            archive_sha256 = _hash_regular_file(args.archive)
            expected = _expected_attestation(
                lock,
                raw,
                platform,
                archive_sha256=archive_sha256,
                distribution_tree=args.distribution_tree,
                locked_wget=args.locked_wget,
                source_builder=args.source_builder,
                build_tree=args.build_tree,
            )
            _write_attestation(args.output, expected)
            return 0
        if args.command == "verify-attestation":
            if args.archive is not None:
                archive_sha256 = _hash_regular_file(args.archive)
            else:
                archive_sha256 = _require_string(
                    args.archive_sha256, "archive SHA-256", _HEX64
                )
            expected = _expected_attestation(
                lock,
                raw,
                platform,
                archive_sha256=archive_sha256,
                distribution_tree=args.distribution_tree,
                locked_wget=args.locked_wget,
                source_builder=args.source_builder,
                build_tree=None,
            )
            actual = _load_attestation(args.attestation)
            if actual != expected:
                raise LockError("source-build attestation does not match the lock and archive")
            return 0
        if args.command == "publish-output-bundle":
            _publish_output_bundle(
                args.archive, args.attestation, raw, args.output_bundle
            )
            return 0
        if args.command == "write-install-state":
            archive_sha256 = _require_string(
                args.archive_sha256, "archive SHA-256", _HEX64
            )
            state = _install_state(
                lock,
                raw,
                platform,
                args.directory,
                args.origin,
                archive_sha256,
                args.attestation,
                args.locked_wget,
                args.source_builder,
            )
            _write_private_json(args.output, state)
            return 0
        if args.command == "verify-install":
            prebuilt_sha256 = _require_string(
                args.prebuilt_sha256, "prebuilt archive SHA-256", _HEX64
            )
            origin = _verify_install(
                lock,
                raw,
                platform,
                args.directory,
                set(args.allowed_origin),
                prebuilt_sha256,
                args.locked_wget,
                args.source_builder,
            )
            print(origin)
            return 0
        if args.command == "publish-install":
            _publish_directory(args.staged, args.destination)
            return 0
        raise AssertionError(f"unhandled command: {args.command}")
    except FileExistsError as error:
        print(f"TLAPM source lock publication race: {error}", file=sys.stderr)
        return 3
    except (LockError, OSError) as error:
        print(f"TLAPM source lock error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
