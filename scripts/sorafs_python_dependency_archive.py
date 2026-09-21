"""Original-byte contracts for the fixed fifteen offline Python dependencies.

This dependency-only owner reuses the sole privacy wheel verifier's archive,
metadata and RECORD primitives. Native/SDK archives retain their own sole parser.
No imports, extraction, installation or process execution occurs here.
"""
from __future__ import annotations

import configparser
from dataclasses import dataclass
import hashlib
import io
from pathlib import PurePosixPath
import re
import zipfile

from sorafs_python_consumer_artifact import ArtifactError, _VERIFIER as verifier, _require
from sorafs_python_dependency_inputs import DependencyWheel, MODULES, MAX_WHEEL_BYTES

MAX_MEMBERS = verifier.MAX_ARCHIVE_MEMBERS
MAX_TOTAL_BYTES = verifier.MAX_TOTAL_UNCOMPRESSED_BYTES
ROOTS = {
    "blake3": ("blake3",), "certifi": ("certifi",), "cffi": ("cffi",),
    "charset-normalizer": ("charset_normalizer",), "cryptography": ("cryptography", "cryptography.libs"),
    "idna": ("idna",), "iniconfig": ("iniconfig",), "packaging": ("packaging",),
    "pip": ("pip",), "pluggy": ("pluggy",), "pycparser": ("pycparser",),
    "pygments": ("pygments",), "pytest": ("_pytest", "pytest", "py.py"),
    "requests": ("requests",), "urllib3": ("urllib3",),
}


def safe_installed_name(name: str) -> None:
    """Keep all startup hooks, cached code and aliased paths out of imported trees."""
    info = zipfile.ZipInfo(name)
    verifier._canonical_zip_member_name(info)
    parts = PurePosixPath(name.rstrip("/")).parts
    _require(not any(part.casefold().split(".", 1)[0] in ("sitecustomize", "usercustomize")
                     for part in parts), "dependency startup module is forbidden")
    _require(not any(part.casefold() == "__pycache__" for part in parts)
             and PurePosixPath(name).suffix.casefold() not in (".pth", ".pyc", ".pyo"),
             "dependency startup hook or cached code is forbidden")


@dataclass(frozen=True)
class DependencyMember:
    """One original archive regular file; installed content must match exactly."""
    name: str
    sha256: str
    size: int


@dataclass(frozen=True)
class DependencyArchive:
    """Verified original bytes and their sole dependency-owned projections."""
    raw: bytes
    wheel: DependencyWheel
    dist_info_root: str
    members: tuple[DependencyMember, ...]
    console_scripts: tuple[str, ...]


def _console_scripts(payload: bytes | None, module: str) -> tuple[str, ...]:
    result = set()
    if payload is not None:
        _require(len(payload) <= 64 * 1024, "dependency entry-point bound")
        parser = configparser.ConfigParser(interpolation=None, strict=True)
        parser.optionxform = str
        parser.read_string(payload.decode("utf-8", "strict"))
        allowed = {"console_scripts"}
        if module == "cffi":
            allowed.add("distutils.setup_keywords")
        _require(not parser.defaults() and set(parser.sections()) <= allowed,
                 "dependency entry-point groups are outside this fixed profile")
        if parser.has_section("distutils.setup_keywords"):
            _require(dict(parser["distutils.setup_keywords"]) == {"cffi_modules": "cffi.setuptools_ext:cffi_modules"},
                     "cffi build metadata differs from the fixed dependency profile")
        if parser.has_section("console_scripts"):
            _require(len(parser["console_scripts"]) <= 32, "dependency console-script count bound")
            for name, value in parser["console_scripts"].items():
                _require(re.fullmatch(r"[A-Za-z][A-Za-z0-9_.-]{0,127}", name) is not None
                         and re.fullmatch(r"[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*:[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*", value) is not None,
                         "dependency console entry point is malformed")
                result.add(name)
    if module == "pip":
        _require("pip" in result, "pinned pip wheel omits its canonical console entry point")
        result.update(("pip3", "pip3.12"))
    _require(len({value.casefold() for value in result}) == len(result), "dependency console scripts alias")
    return tuple(sorted(result))


def parse_dependency_wheel(raw: bytes, *, wheel: DependencyWheel) -> DependencyArchive:
    """Validate only fixed dependency originals using the existing ZIP/RECORD owner."""
    _require(type(wheel) is DependencyWheel and wheel.module in MODULES,
             "dependency archive has no fixed manifest owner")
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_WHEEL_BYTES
             and len(raw) == wheel.file.size and hashlib.sha256(raw).hexdigest() == wheel.file.sha256,
             "dependency original bytes differ from independent manifest pin")
    dist = wheel.module.replace("-", "_") + "-" + wheel.version + ".dist-info"
    try:
        verifier.preflight_zip_directory(
            raw, max_members=MAX_MEMBERS,
            max_name_bytes=verifier.MAX_MEMBER_NAME_BYTES,
            allow_member_extra=True, allow_member_comments=True,
        )
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            infos = archive.infolist()
            _require(0 < len(infos) <= MAX_MEMBERS, "dependency archive member bound")
            verifier._assert_canonical_zip_envelope(raw, archive, infos)
            names, aliases, total, offsets = {}, set(), 0, set()
            for info in infos:
                name = verifier._canonical_zip_member_name(info)
                safe_installed_name(name)
                key = name.rstrip("/").casefold()
                _require(key not in aliases, "dependency archive aliases members")
                aliases.add(key)
                verifier._assert_safe_zip_mode(info, name)
                _require(not info.flag_bits & 1 and info.compress_type in verifier.ALLOWED_COMPRESSION,
                         "dependency archive encryption/compression policy")
                _require(0 <= info.file_size <= verifier.MAX_MEMBER_BYTES
                         and 0 <= info.compress_size <= MAX_WHEEL_BYTES,
                         "dependency member size bound")
                _require(info.file_size == 0 or info.compress_size > 0,
                         "dependency member has an invalid compressed size")
                _require(info.compress_size == 0 or info.file_size <= info.compress_size * verifier.MAX_COMPRESSION_RATIO,
                         "dependency compression ratio bound")
                total += info.file_size
                _require(total <= MAX_TOTAL_BYTES, "dependency uncompressed byte bound")
                _require(0 <= info.header_offset < archive.start_dir and info.header_offset not in offsets,
                         "dependency archive local-header ownership differs")
                offsets.add(info.header_offset)
                root = name.split("/", 1)[0]
                cffi_native = (wheel.module == "cffi" and "/" not in name
                               and re.fullmatch(r"_cffi_backend\.[A-Za-z0-9_.-]+\.so", name) is not None)
                _require(root == dist or root in ROOTS[wheel.module] or cffi_native,
                         "dependency archive contains an unowned or relocated root")
                names[name] = info
            # Implicit directories have the same casing and cannot shadow regular files.
            spellings = {}
            for name, info in names.items():
                path = PurePosixPath(name.rstrip("/"))
                for part in (path, *path.parents):
                    if str(part) == ".":
                        continue
                    key = str(part).casefold()
                    _require(key not in spellings or spellings[key] == str(part), "dependency path ancestors alias")
                    spellings[key] = str(part)
                _require(not any(str(parent) in names and not names[str(parent)].is_dir()
                                 for parent in path.parents), "dependency file is also an ancestor")
            mandatory = {dist + "/" + name for name in verifier.DIST_INFO_REQUIRED_FILES}
            _require(mandatory <= set(names) and all(not names[name].is_dir() for name in mandatory),
                     "dependency dist-info omits required regular files")
            _require(not {dist + "/" + name for name in verifier.PIP_GENERATED_DIST_INFO_FILES} & set(names),
                     "dependency archive preseeds installed metadata")
            metadata = verifier._read_member_bytes(archive, names[dist + "/METADATA"], label="dependency METADATA")
            _, version = verifier._metadata_identity(metadata, verifier.WheelOwner(wheel.module.replace("-", "_"), wheel.module, False))
            _require(version == wheel.version, "dependency METADATA version differs from independent pin")
            wheel_metadata = verifier._read_member_bytes(archive, names[dist + "/WHEEL"], label="dependency WHEEL").decode("utf-8", "strict")
            headers = wheel_metadata.splitlines()
            _require([line for line in headers if line.casefold().startswith("wheel-version:")] == ["Wheel-Version: 1.0"],
                     "dependency wheel layout is not version 1.0")
            _require([line for line in headers if line.casefold().startswith("root-is-purelib:")]
                     in (["Root-Is-Purelib: true"], ["Root-Is-Purelib: false"]), "dependency install scheme differs")
            members = tuple(DependencyMember(name, verifier._stream_member_digest(archive, info), info.file_size)
                            for name, info in sorted(names.items()) if not info.is_dir())
            record_name = dist + "/RECORD"
            record = verifier._read_member_bytes(archive, names[record_name], label="dependency RECORD")
            verifier._assert_record_payload(record, expected_files={member.name: (member.sha256, member.size)
                                            for member in members if member.name != record_name},
                                            record_name=record_name, label="original dependency RECORD")
            entry_name = dist + "/entry_points.txt"
            entry = (verifier._read_member_bytes(archive, names[entry_name], label="dependency entry points")
                     if entry_name in names else None)
            scripts = _console_scripts(entry, wheel.module)
    except (verifier.VerificationError, zipfile.BadZipFile, configparser.Error, NotImplementedError) as error:
        raise ArtifactError("dependency archive is not a valid bounded wheel: " + str(error)) from error
    return DependencyArchive(raw, wheel, dist, members, scripts)
