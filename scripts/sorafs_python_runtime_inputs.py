"""Pinned POSIX CPython 3.12 input inventories and exact bounded byte bundles.

Library only: consumes independently supplied manifest digests and captured
bytes. No runtime discovery, process execution, installation, network or output
path mutation. A parsed bundle authenticates content joins, not a running host.
Normal system/native-library provenance remains platform qualification.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from pathlib import PurePosixPath
import re
import struct

from sorafs_evidence_json import decode_evidence_json
from sorafs_python_consumer_artifact import ArtifactError, canonical_json, _digest, _path, _require
from sorafs_sdk_artifact_index import FileReference

SCHEMA = "sorafs.python.runtime_inputs.v1"
MAGIC = b"SORAFS_PYTHON_RUNTIME_INPUTS_V1\n"
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_FILES = 8192
MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_RUNTIME_BYTES = 512 * 1024 * 1024
MAX_BUNDLE_BYTES = len(MAGIC) + 8 + MAX_MANIFEST_BYTES + MAX_RUNTIME_BYTES


def closed(value: object, names: set[str], label: str) -> dict:
    """Reject unknown/missing first-release fields."""
    _require(type(value) is dict and set(value) == names, label + " fields differ")
    return value


def pinned_json(raw: bytes, expected_sha256: str, maximum: int) -> dict:
    """Check the independent digest before decoding one canonical input manifest."""
    _digest(expected_sha256)
    _require(type(raw) is bytes and 0 < len(raw) <= maximum, "manifest byte bound")
    _require(hashlib.sha256(raw).hexdigest() == expected_sha256, "independent manifest pin differs")
    value = decode_evidence_json(raw)
    _require(canonical_json(value) == raw, "manifest JSON is not canonical")
    return value


def file_reference(value: object, *, absolute: bool) -> FileReference:
    """Validate a bounded file identity without claiming any filesystem owner."""
    row = closed(value, {"path", "sha256", "size"}, "file")
    path, digest, size = _path(row["path"], absolute=absolute), _digest(row["sha256"]), row["size"]
    _require(type(size) is int and 0 <= size <= MAX_FILE_BYTES, "file size bound")
    return FileReference(path, digest, size)


@dataclass(frozen=True)
class RuntimeLink:
    """One explicit stock-toolchain file alias to a pinned regular file."""
    path: str
    target: str
    resolved: str


@dataclass(frozen=True)
class RuntimeManifest:
    """Independent inventory; claims require live custody or exact bundle verification."""
    raw: bytes
    sha256: str
    platform: str
    version: str
    executable: FileReference
    shared_runtime: tuple[FileReference, ...]
    stdlib_root: str
    stdlib_files: tuple[FileReference, ...]
    stdlib_directories: tuple[str, ...]
    stdlib_links: tuple[RuntimeLink, ...]
    zip_path: str
    zip_file: FileReference | None
    site_packages_kind: str
    site_packages_target: str | None

    def files(self) -> tuple[FileReference, ...]:
        """Return the sole bundle order, preserving every absolute input location."""
        stdlib = tuple(FileReference(str(PurePosixPath(self.stdlib_root) / row.path), row.sha256, row.size)
                       for row in self.stdlib_files)
        return (self.executable, *self.shared_runtime,
                *((self.zip_file,) if self.zip_file is not None else ()), *stdlib)



def _validate_link_resolution(manifest: RuntimeManifest) -> None:
    """Derive every alias target using only the complete pinned path inventory."""
    files = {value.path for value in manifest.files()}
    links = {str(PurePosixPath(manifest.stdlib_root) / value.path): value
             for value in manifest.stdlib_links}
    directories = {manifest.stdlib_root, *(str(PurePosixPath(manifest.stdlib_root) / value)
                                          for value in manifest.stdlib_directories)}
    for path in (*files, *links, *tuple(directories)):
        directories.update(str(parent) for parent in PurePosixPath(path).parents)
    for path, link in links.items():
        pending = path.split("/")[1:]
        resolved, followed = [], 0
        while pending:
            part = pending.pop(0)
            if part in ("", "."):
                continue
            if part == "..":
                if resolved:
                    resolved.pop()
                continue
            current = "/" + "/".join((*resolved, part))
            if current in links:
                followed += 1
                _require(followed <= 64, "runtime symlink cycle/depth bound")
                target = links[current].target
                if target.startswith("/"):
                    resolved = []
                pending = target.split("/") + pending
            else:
                _require(current in directories or (not pending and current in files),
                         "runtime symlink traverses an unpinned or non-directory member")
                resolved.append(part)
        _require("/" + "/".join(resolved) == link.resolved,
                 "runtime symlink target does not resolve to its pinned member")


def parse_runtime_manifest(raw: bytes, *, expected_sha256: str) -> RuntimeManifest:
    """Validate the complete fixed host input shape; never infer current-machine pins."""
    row = closed(pinned_json(raw, expected_sha256, MAX_MANIFEST_BYTES),
                 {"schema", "platform", "version", "executable", "shared_runtime", "stdlib",
                  "stdlib_zip", "site_packages"}, "runtime manifest")
    _require(row["schema"] == SCHEMA and row["platform"] in ("darwin", "linux"), "runtime profile differs")
    _require(type(row["version"]) is str and re.fullmatch(r"3\.12\.(?:0|[1-9][0-9]*)", row["version"]) is not None,
             "runtime version must be exact stable CPython 3.12")
    executable = file_reference(row["executable"], absolute=True)
    _require(executable.size > 0, "runtime executable is empty")
    shared = row["shared_runtime"]
    _require(type(shared) is list and len(shared) <= 16, "shared runtime inventory bound")
    shared = tuple(file_reference(value, absolute=True) for value in shared)
    _require(all(value.size > 0 for value in shared), "shared runtime is empty")
    _require(tuple(value.path for value in shared) == tuple(sorted({value.path for value in shared})), "shared runtime order differs")
    stdlib = closed(row["stdlib"], {"root", "files", "links", "directories"}, "stdlib")
    root = _path(stdlib["root"], absolute=True)
    _require(PurePosixPath(root).name == "python3.12", "stdlib root must be configured python3.12")
    _require(type(stdlib["files"]) is list and 0 < len(stdlib["files"]) <= MAX_FILES, "stdlib file count bound")
    files = tuple(file_reference(value, absolute=False) for value in stdlib["files"])
    names = tuple(value.path for value in files)
    _require(names == tuple(sorted(set(names))), "stdlib files are not sorted/unique")
    _require({"os.py", "site.py", "sysconfig.py", "encodings/__init__.py"} <= set(names), "stdlib inventory omits mandatory roots")
    _require(type(stdlib["directories"]) is list and len(stdlib["directories"]) <= MAX_FILES, "stdlib directory bound")
    directories = tuple(_path(value, absolute=False) for value in stdlib["directories"])
    _require(directories == tuple(sorted(set(directories))), "stdlib directories are not sorted/unique")
    _require(type(stdlib["links"]) is list and len(stdlib["links"]) <= 64, "stdlib link count bound")
    links = []
    for value in stdlib["links"]:
        value = closed(value, {"path", "target", "resolved"}, "stdlib link")
        target = value["target"]
        _require(type(target) is str and 0 < len(target.encode("utf-8")) <= 4096
                 and "\\" not in target and not any(ord(char) < 32 or ord(char) == 127 for char in target), "invalid link target")
        links.append(RuntimeLink(_path(value["path"], absolute=False), target, _path(value["resolved"], absolute=True)))
    _require(tuple(link.path for link in links) == tuple(sorted({link.path for link in links})), "stdlib links are not sorted/unique")
    all_names = set(names) | {link.path for link in links}
    _require(len(all_names) == len(names) + len(links) and len(all_names) <= MAX_FILES, "stdlib aliases a file/link")
    _require(all(name != "site-packages" and not name.startswith("site-packages/") for name in all_names), "base site-packages is not runtime custody")
    _require(all(name != "site-packages" and not name.startswith("site-packages/") for name in directories), "excluded site-packages directory was included")
    _require(not all_names.intersection(directories), "stdlib directory aliases a member")
    _require(all(str(parent) in directories for name in (*all_names, *directories)
                 for parent in PurePosixPath(name).parents if str(parent) != "."), "stdlib parent directory is absent")
    _require(not any(name.split("/", 1)[0].casefold().split(".", 1)[0]
                     in {"sitecustomize", "usercustomize"}
                     for name in (*all_names, *directories)),
             "runtime startup customization is forbidden")
    _require(not any(str(parent) in all_names for name in all_names for parent in PurePosixPath(name).parents), "stdlib file/directory collision")
    zip_row = closed(row["stdlib_zip"], {"path", "sha256", "size"}, "stdlib zip slot")
    zip_path = _path(zip_row["path"], absolute=True)
    _require(zip_path == str(PurePosixPath(root).parent / "python312.zip"), "stdlib zip slot differs")
    zip_file = None
    if zip_row["sha256"] is None:
        _require(zip_row["size"] is None, "absent zip must have no byte claim")
    else:
        zip_file = file_reference(zip_row, absolute=True)
        _require(zip_file.size > 0, "stdlib zip is empty")
    site = closed(row["site_packages"], {"kind", "target"}, "excluded site-packages slot")
    _require(site["kind"] in ("absent", "directory", "symlink"), "excluded site-packages kind differs")
    if site["kind"] == "symlink":
        _require(type(site["target"]) is str and 0 < len(site["target"].encode()) <= 4096 and "\0" not in site["target"], "excluded site-packages target differs")
    else:
        _require(site["target"] is None, "excluded non-link site-packages has a target")
    manifest = RuntimeManifest(raw, expected_sha256, row["platform"], row["version"], executable,
                               shared, root, files, directories, tuple(links), zip_path, zip_file, site["kind"], site["target"])
    every = manifest.files()
    _require(len({value.path for value in every}) == len(every), "runtime file paths alias")
    _require(sum(value.size for value in every) <= MAX_RUNTIME_BYTES, "runtime aggregate byte bound")
    _require(all(link.resolved in {value.path for value in every} for link in links), "stdlib link escapes pinned runtime files")
    _validate_link_resolution(manifest)
    return manifest


@dataclass(frozen=True)
class RuntimeBundle:
    """Original captured bytes plus verified file offsets, with no live execution claim."""
    raw: bytes
    manifest: RuntimeManifest
    offsets: tuple[tuple[str, int, int], ...]

    def member_bytes(self, path: str) -> bytes:
        """Return only a declared, authenticated member from these original bytes."""
        for member, offset, size in self.offsets:
            if member == path:
                return self.raw[offset:offset + size]
        raise ArtifactError("runtime bundle member is absent")


def parse_runtime_bundle(raw: bytes, *, expected_manifest_sha256: str) -> RuntimeBundle:
    """Check the fixed length-framed format, exact inventory, every digest and EOF."""
    header = len(MAGIC) + 8
    _require(type(raw) is bytes and header < len(raw) <= MAX_BUNDLE_BYTES and raw.startswith(MAGIC), "runtime bundle envelope differs")
    length = struct.unpack_from(">Q", raw, len(MAGIC))[0]
    _require(0 < length <= MAX_MANIFEST_BYTES and header + length <= len(raw), "runtime manifest length bound")
    manifest = parse_runtime_manifest(raw[header:header + length], expected_sha256=expected_manifest_sha256)
    offset = header + length
    _require(offset + sum(value.size for value in manifest.files()) == len(raw), "runtime bundle has missing or trailing bytes")
    offsets = []
    for value in manifest.files():
        body = memoryview(raw)[offset:offset + value.size]
        _require(hashlib.sha256(body).hexdigest() == value.sha256, "runtime bundle member digest differs")
        offsets.append((value.path, offset, value.size))
        offset += value.size
    return RuntimeBundle(raw, manifest, tuple(offsets))
