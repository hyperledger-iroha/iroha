#!/usr/bin/env python3
"""Render a checksum-pinned CocoaPods spec for one NoritoBridge release ZIP."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
from typing import NoReturn
import zipfile


ARCHIVE_ROOT = "NoritoBridge.xcframework"
EMBEDDED_MANIFEST = f"{ARCHIVE_ROOT}/NoritoBridge.artifacts.json"
PLACEHOLDERS = ("__VERSION__", "__SOURCE_URL__", "__ARCHIVE_SHA256__")
SEMVER = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\Z")
CHUNK_SIZE = 1024 * 1024
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_ARCHIVE_ENTRIES = 4096
MAX_ENTRY_BYTES = 256 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_BYTES = 1024 * 1024 * 1024
MAX_MANIFEST_BYTES = 64 * 1024
MAX_TEMPLATE_BYTES = 64 * 1024


class RenderError(RuntimeError):
    """The podspec inputs or destination violate the release contract."""


def fail(message: str) -> NoReturn:
    raise RenderError(message)


def canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        fail(f"{label} must be an absolute canonical directory")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"unable to inspect {label}: {error}")
    if resolved != path or stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        fail(f"{label} must be a non-symbolic canonical directory")
    return path


def private_output_directory(path: Path) -> Path:
    path = canonical_directory(path, "podspec output parent")
    metadata = path.lstat()
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
        fail("podspec output parent must be current-UID-owned with exact mode 0700")
    if not os.access(path, os.W_OK | os.X_OK):
        fail("podspec output parent must be writable and searchable")
    return path


def canonical_regular_file(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        fail(f"{label} must be an absolute canonical file")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"unable to inspect {label}: {error}")
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
    ):
        fail(f"{label} must be a non-symbolic canonical single-link regular file")
    return path


def read_regular(path: Path, label: str, *, max_bytes: int) -> bytes:
    path = canonical_regular_file(path, label)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"unable to open {label}: {error}")
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.geteuid()
            or stat.S_IMODE(before.st_mode) & 0o022
        ):
            fail(
                f"{label} must remain a current-UID-owned, non-writable-by-others, "
                "single-link regular file"
            )
        if before.st_size > max_bytes:
            fail(f"{label} exceeds the {max_bytes}-byte limit")
        chunks: list[bytes] = []
        total_bytes = 0
        while chunk := os.read(descriptor, CHUNK_SIZE):
            total_bytes += len(chunk)
            if total_bytes > max_bytes:
                fail(f"{label} exceeds the {max_bytes}-byte limit")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        visible = path.lstat()
    finally:
        os.close(descriptor)
    identity = (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_uid,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    payload = b"".join(chunks)
    if identity != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_uid,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ) or identity != (
        visible.st_dev,
        visible.st_ino,
        visible.st_mode,
        visible.st_nlink,
        visible.st_uid,
        visible.st_size,
        visible.st_mtime_ns,
        visible.st_ctime_ns,
    ) or len(payload) != before.st_size:
        fail(f"{label} changed while it was being read")
    return payload


def validate_archive(path: Path, expected_version: str) -> bytes:
    payload = read_regular(
        path,
        "NoritoBridge archive",
        max_bytes=MAX_ARCHIVE_BYTES,
    )
    try:
        with zipfile.ZipFile(io.BytesIO(payload)) as archive:
            entries = archive.infolist()
            if not entries:
                fail("NoritoBridge archive is empty")
            if len(entries) > MAX_ARCHIVE_ENTRIES:
                fail(
                    "NoritoBridge archive exceeds the "
                    f"{MAX_ARCHIVE_ENTRIES}-entry limit"
                )
            names: set[str] = set()
            casefolded_names: set[str] = set()
            total_uncompressed = 0
            for entry in entries:
                name = entry.filename
                components = name.rstrip("/").split("/")
                casefolded = name.rstrip("/").casefold()
                if (
                    not name
                    or "\x00" in name
                    or "\\" in name
                    or name.startswith("/")
                    or not components
                    or components[0] != ARCHIVE_ROOT
                    or any(part in {"", ".", ".."} for part in components)
                    or name in names
                    or casefolded in casefolded_names
                ):
                    fail(
                        "unsafe, duplicate, or case-colliding NoritoBridge "
                        f"archive entry: {name!r}"
                    )
                names.add(name)
                casefolded_names.add(casefolded)
                if entry.flag_bits & 0x1:
                    fail(f"encrypted NoritoBridge archive entries are forbidden: {name}")
                if entry.compress_type != zipfile.ZIP_STORED:
                    fail(f"NoritoBridge archive entry is not deterministically stored: {name}")
                unix_mode = entry.external_attr >> 16
                file_type = stat.S_IFMT(unix_mode)
                if file_type == stat.S_IFLNK:
                    fail(f"symbolic links are forbidden in NoritoBridge archives: {name}")
                if file_type not in {0, stat.S_IFREG, stat.S_IFDIR}:
                    fail(f"unsupported NoritoBridge archive entry type: {name}")
                if entry.file_size > MAX_ENTRY_BYTES:
                    fail(
                        f"NoritoBridge archive entry exceeds the {MAX_ENTRY_BYTES}-byte "
                        f"limit: {name}"
                    )
                total_uncompressed += entry.file_size
                if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_BYTES:
                    fail(
                        "NoritoBridge archive exceeds the "
                        f"{MAX_TOTAL_UNCOMPRESSED_BYTES}-byte uncompressed limit"
                    )
            if EMBEDDED_MANIFEST not in names:
                fail(f"NoritoBridge archive is missing {EMBEDDED_MANIFEST}")
            manifest = archive.getinfo(EMBEDDED_MANIFEST)
            if not 2 <= manifest.file_size <= MAX_MANIFEST_BYTES:
                fail("embedded NoritoBridge manifest must contain 2..65536 bytes")
            bad_crc = archive.testzip()
            if bad_crc is not None:
                fail(f"NoritoBridge archive has a corrupt entry: {bad_crc}")
            try:
                manifest_document = json.loads(archive.read(manifest))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                fail(f"embedded NoritoBridge manifest is not JSON: {error}")
            if (
                not isinstance(manifest_document, dict)
                or manifest_document.get("version") != expected_version
            ):
                fail(
                    "embedded NoritoBridge manifest version must equal "
                    f"IrohaSwift/VERSION ({expected_version})"
                )
    except (
        OSError,
        RuntimeError,
        NotImplementedError,
        zipfile.BadZipFile,
        zipfile.LargeZipFile,
    ) as error:
        fail(f"unable to authenticate NoritoBridge archive: {error}")
    return payload


def parse_version(root: Path) -> str:
    raw = read_regular(
        root / "IrohaSwift/VERSION",
        "IrohaSwift VERSION",
        max_bytes=64,
    )
    try:
        version = raw.decode("ascii").strip()
    except UnicodeDecodeError as error:
        fail(f"IrohaSwift VERSION must be ASCII: {error}")
    if raw != f"{version}\n".encode("ascii") or SEMVER.fullmatch(version) is None:
        fail("IrohaSwift VERSION must contain one canonical SemVer and a newline")
    return version


def render(root: Path, archive: Path, *, local_source: bool) -> bytes:
    version = parse_version(root)
    archive_payload = validate_archive(archive, version)
    digest = hashlib.sha256(archive_payload).hexdigest()
    template_path = root / "crates/connect_norito_bridge/NoritoBridge.podspec.template"
    template_payload = read_regular(
        template_path,
        "NoritoBridge podspec template",
        max_bytes=MAX_TEMPLATE_BYTES,
    )
    try:
        template = template_payload.decode("utf-8")
    except UnicodeDecodeError as error:
        fail(f"NoritoBridge podspec template must be UTF-8: {error}")
    for placeholder in PLACEHOLDERS:
        if template.count(placeholder) != 1:
            fail(f"podspec template must contain {placeholder} exactly once")
    if local_source:
        source_url = archive.as_uri()
    else:
        source_url = (
            "https://github.com/hyperledger-iroha/iroha/releases/download/"
            f"v{version}/NoritoBridge-v{version}.xcframework.zip"
        )
    rendered = template
    for placeholder, value in (
        ("__VERSION__", version),
        ("__SOURCE_URL__", source_url),
        ("__ARCHIVE_SHA256__", digest),
    ):
        rendered = rendered.replace(placeholder, value)
    if any(placeholder in rendered for placeholder in PLACEHOLDERS):
        fail("podspec template rendering left an unresolved placeholder")
    return rendered.encode("utf-8")


def file_identity(metadata: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
    )


def output_identity(path: Path) -> tuple[int, int, int, int, int, int]:
    metadata = path.lstat()
    return file_identity(metadata)


def same_owned_file(
    actual: tuple[int, int, int, int, int, int],
    expected: tuple[int, int, int, int, int, int],
) -> bool:
    return actual[:3] == expected[:3] and actual[4:] == expected[4:]


def publish_no_replace(output: Path, payload: bytes) -> None:
    if not output.is_absolute() or output != Path(os.path.abspath(output)):
        fail("podspec output must be an absolute canonical filename")
    parent = private_output_directory(output.parent)
    output = parent / output.name
    if output.name in {"", ".", ".."} or output.suffix != ".podspec":
        fail("podspec output must be a .podspec filename")
    try:
        output.lstat()
    except FileNotFoundError:
        pass
    except OSError as error:
        fail(f"unable to inspect podspec output: {error}")
    else:
        fail(f"podspec output must not already exist: {output}")

    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.", suffix=".tmp", dir=parent
    )
    temporary = Path(temporary_name)
    owned_identity: tuple[int, int, int, int, int, int] | None = None
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fchmod(handle.fileno(), 0o644)
            os.fsync(handle.fileno())
        owned_identity = output_identity(temporary)
        try:
            os.link(temporary, output, follow_symlinks=False)
        except FileExistsError:
            fail(f"podspec output must not already exist: {output}")
        if not same_owned_file(output_identity(output), owned_identity):
            fail("published podspec output did not retain the staged inode")
        temporary.unlink()
        directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
        directory_flags |= getattr(os, "O_NOFOLLOW", 0)
        directory_descriptor = os.open(parent, directory_flags)
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
        expected_mode = stat.S_IFREG | 0o644
        visible_identity = output_identity(output)
        if (
            visible_identity != owned_identity
            or visible_identity[2] != expected_mode
            or visible_identity[3] != 1
            or visible_identity[4] != os.geteuid()
            or visible_identity[5] != len(payload)
            or read_regular(
                output,
                "published podspec output",
                max_bytes=MAX_TEMPLATE_BYTES,
            )
            != payload
        ):
            fail("published podspec output failed final authentication")
    except BaseException:
        try:
            temporary.unlink(missing_ok=True)
        except BaseException:
            pass
        if owned_identity is not None:
            try:
                if same_owned_file(output_identity(output), owned_identity):
                    output.unlink()
            except BaseException:
                pass
        raise


def production_filename(version: str) -> str:
    return f"NoritoBridge-v{version}.xcframework.zip"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--local-source",
        action="store_true",
        help="render an explicit file:// source for an offline lint only",
    )
    arguments = parser.parse_args()
    root = canonical_directory(arguments.root, "repository root")
    archive = canonical_regular_file(arguments.archive, "NoritoBridge archive")
    if root == archive or root in archive.parents:
        fail("NoritoBridge archive must be outside the repository")
    if root == arguments.output or root in arguments.output.parents:
        fail("podspec output must be outside the repository")
    version = parse_version(root)
    if not arguments.local_source and archive.name != production_filename(version):
        fail(
            "production archive filename must be "
            f"{production_filename(version)}"
        )
    payload = render(root, archive, local_source=arguments.local_source)
    publish_no_replace(arguments.output, payload)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RenderError as error:
        print(f"render_norito_bridge_podspec.py: error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
