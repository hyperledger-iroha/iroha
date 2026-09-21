"""Join installed offline dependency bytes before any installed Python startup.

Consumes original fixed dependency archives and complete captured environment
bytes. Native/SDK installed content comes from the existing sole verifier. This is
a byte-origin join; process observation and file-lifetime custody stay with the
producer. Generated console programs are retained output, never an execution
input: the process owner keeps the private environment bin directory off PATH.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from pathlib import Path, PurePosixPath
from typing import Mapping

from sorafs_python_consumer_artifact import _VERIFIER as verifier, _require
from sorafs_python_dependency_archive import (
    DependencyArchive, parse_dependency_wheel, safe_installed_name,
)
from sorafs_python_dependency_inputs import MODULES

SITE = "lib/python3.12/site-packages/"
MAX_ENVIRONMENT_BYTES = 512 * 1024 * 1024
MAX_ENVIRONMENT_FILES = 16000


def _identity(raw: bytes) -> tuple[str, int]:
    return hashlib.sha256(raw).hexdigest(), len(raw)


@dataclass(frozen=True)
class InstalledDependencyMember:
    """An authenticated original or retained generated relative environment file."""
    module: str
    path: str
    sha256: str
    size: int
    generated: bool


@dataclass(frozen=True)
class InstalledDependencySet:
    """Exact dependency file joins; no inferred process success or release authority."""
    files: tuple[InstalledDependencyMember, ...]


def _native_sdk_paths(installed, files: dict[str, bytes]) -> set[str]:
    _require(type(installed) is tuple and len(installed) == 2,
             "exact native and SDK installed content is required")
    observed, roots = set(), set()
    for content in installed:
        _require(type(content) is verifier.InstalledWheelContent,
                 "native/SDK requires the sole verifier's installed content")
        package = content.owner.package
        _require(content.owner in (verifier.NATIVE_OWNER, verifier.SDK_OWNER) and package not in roots
                 and content.package.name == package + "/__init__.py"
                 and (content.native is not None) == (package == "iroha_native"),
                 "native/SDK installed root ownership differs")
        roots.add(package)
        _require(content.package in content.files and (content.native is None or content.native in content.files),
                 "native/SDK installed content omits its entry points")
        for value in content.files:
            top = value.name.split("/", 1)[0]
            _require(top == package or (top.startswith(package + "-") and top.endswith(".dist-info")),
                     "native/SDK installed content covers a foreign module")
            key = SITE + value.name
            _require(key not in observed and key in files and _identity(files[key]) == (value.sha256, value.size),
                     "captured native/SDK bytes differ from the sole installed content")
            observed.add(key)
    return observed


def verify_dependency_install(
    archives: tuple[DependencyArchive, ...], environment_files: dict[str, bytes], *,
    environment: Path, wheel_paths_by_module: Mapping[str, Path],
    native_sdk_content: tuple,
) -> InstalledDependencySet:
    """Authenticate every dependency before the parent starts distribution/pytest code.

    `environment_files` is the parent's complete descriptor-authenticated capture,
    and `native_sdk_content` contains the sole verifier's two byte-relation
    results, obtained directly or through each live result's `.content`. There is no user-supplied path exclusion or skip option.
    """
    _require(type(archives) is tuple and tuple(value.wheel.module for value in archives) == MODULES,
             "installed dependency archive inventory differs")
    _require(type(environment_files) is dict and len(environment_files) <= MAX_ENVIRONMENT_FILES,
             "installed environment file bound")
    _require(all(type(raw) is bytes and len(raw) <= verifier.MAX_MEMBER_BYTES for raw in environment_files.values())
             and sum(map(len, environment_files.values())) <= MAX_ENVIRONMENT_BYTES,
             "installed environment byte bound")
    _require(type(environment) is type(Path()) and environment.is_absolute()
             and str(environment) == str(PurePosixPath(environment)) and ".." not in environment.parts,
             "installed environment must be the exact absolute producer root")
    _require(set(wheel_paths_by_module) == set(MODULES)
             and len(set(wheel_paths_by_module.values())) == len(MODULES), "installed wheel location mapping differs")
    aliases = {}
    for name in environment_files:
        safe_installed_name(name)
        _require(name.casefold() not in aliases, "installed environment paths alias")
        aliases[name.casefold()] = name
    native_paths = _native_sdk_paths(native_sdk_content, environment_files)
    claimed, scripts, rows = {}, {}, []

    def claim(module: str, path: str, digest: str, size: int, *, generated: bool) -> None:
        _require(path in environment_files and _identity(environment_files[path]) == (digest, size),
                 "installed dependency member differs from its original bytes")
        _require(path.casefold() not in claimed and path not in native_paths,
                 "installed dependency ownership collides")
        claimed[path.casefold()] = path
        rows.append(InstalledDependencyMember(module, path, digest, size, generated))

    for archive in archives:
        _require(type(archive) is DependencyArchive
                 and parse_dependency_wheel(archive.raw, wheel=archive.wheel) == archive,
                 "dependency archive projection differs from original bytes")
        module, dist = archive.wheel.module, archive.dist_info_root
        wheel_path = wheel_paths_by_module[module]
        _require(type(wheel_path) is type(Path()) and wheel_path.is_absolute()
                 and ".." not in wheel_path.parts and wheel_path.suffix == ".whl",
                 "installed dependency wheel location is not canonical")
        expected = {}
        record_name = dist + "/RECORD"
        for member in archive.members:
            if member.name != record_name:
                claim(module, SITE + member.name, member.sha256, member.size, generated=False)
                expected[member.name] = member.sha256, member.size
        for basename, raw in (("INSTALLER", b"pip\n"), ("REQUESTED", b"")):
            name = dist + "/" + basename
            expected[name] = _identity(raw)
            claim(module, SITE + name, *expected[name], generated=True)
        direct_name = dist + "/direct_url.json"
        _require(SITE + direct_name in environment_files, "installed dependency direct URL is absent")
        direct = environment_files[SITE + direct_name]
        verifier._assert_direct_url(direct, source_uri=wheel_path.as_uri(),
                                    wheel_sha256=archive.wheel.file.sha256)
        expected[direct_name] = _identity(direct)
        claim(module, SITE + direct_name, *expected[direct_name], generated=True)
        for name in archive.console_scripts:
            path = "bin/" + name
            _require(name.casefold() not in scripts and path in environment_files,
                     "installed console script is missing or has multiple owners")
            scripts[name.casefold()] = module
            body = environment_files[path]
            _require(0 < len(body) <= 1024 * 1024, "generated console script byte bound")
            expected["../../../" + path] = _identity(body)
            claim(module, path, *_identity(body), generated=True)
        _require(SITE + record_name in environment_files, "installed dependency RECORD is absent")
        record = environment_files[SITE + record_name]
        verifier._assert_record_payload(record, expected_files=expected, record_name=record_name,
                                        label="installed dependency RECORD")
        claim(module, SITE + record_name, *_identity(record), generated=True)
    site_observed = {name for name in environment_files if name.startswith(SITE)}
    site_claimed = {name for name in claimed.values() if name.startswith(SITE)}
    _require(site_observed == site_claimed | native_paths,
             "installed site contains unowned dependency/package metadata")
    return InstalledDependencySet(tuple(sorted(rows, key=lambda row: row.path)))
