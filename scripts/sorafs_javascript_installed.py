"""Exact inert node_modules content from the sole original npm/source owners.

The caller authenticates and holds the original inputs. Results express only a
byte/mode/location relation, never npm execution, loaded code or qualification.
The node_modules root excludes separately owned environment/qualification files.
The one generated npm hidden lock is mandatory and independently joined to
original graph/content; no generated file is ignored. Native addon/checksum
inputs selected via the existing IROHA_JS_NATIVE_DIR stay under a separate
original native/ABI owner, outside this portable installed tree.
"""
from __future__ import annotations

from dataclasses import dataclass

from sorafs_javascript_archive import MAX_MEMBER_BYTES, MAX_NAME_BYTES, NpmPathInventory, _require
from sorafs_javascript_dependencies import DependencyLock, parse_dependency_archives
from sorafs_javascript_package_source import MAX_SOURCE_FILES, verify_package_content
from sorafs_javascript_install_metadata import HIDDEN_LOCK, verify_npm_hidden_lock

MAX_INSTALLED_FILES = 8192
MAX_INSTALLED_BYTES = 192 * 1024 * 1024
MAX_DIRECTORY_DEPTH = 64
SDK_LOCATION = "node_modules/@iroha/iroha-js"


@dataclass(frozen=True)
class InstalledMember:
    """One exact regular path beneath node_modules, with its original owner."""
    path: str
    owner: str
    archive_member: str | None
    content: bytes
    mode: int


@dataclass(frozen=True)
class InstalledProjection:
    """Frozen original bytes plus their rederivable installed content relation."""
    package_raw: bytes
    sources: tuple[tuple[str, bytes], ...]
    checksum_manifest: bytes
    lock: DependencyLock
    dependency_originals: tuple[tuple[str, bytes], ...]
    npm_hidden_lock: bytes
    environment_label: str
    sdk_archive_label: str
    members: tuple[InstalledMember, ...]

    def files(self) -> dict[str, bytes]:
        """Return exact content labels without opening any producer pathname."""
        return {row.path: row.content for row in self.members}


def installed_projection(package_raw: bytes, *, sources: dict[str, bytes],
                         checksum_manifest: bytes, lock: DependencyLock,
                         dependency_originals: dict[str, bytes], npm_hidden_lock: bytes,
                         environment_label: str, sdk_archive_label: str) -> InstalledProjection:
    """Join the original SDK and nine dependencies at exact resolution locations.

    Source completeness/authentication stays with the caller's original owner.
    Every parser is the existing shared byte owner. In particular neither nested
    hashes2 nor the package checksum can be supplied by installed-tree claims.
    """
    package = verify_package_content(package_raw, sources=sources,
                                     checksum_manifest=checksum_manifest, lock=lock)
    dependencies = parse_dependency_archives(lock, dependency_originals)
    verify_npm_hidden_lock(npm_hidden_lock, archive=package.archive, lock=lock,
                           environment_label=environment_label, sdk_archive_label=sdk_archive_label)
    inventory = NpmPathInventory()
    inventory.admit(HIDDEN_LOCK)
    rows = [InstalledMember(HIDDEN_LOCK, "@npm-generated-hidden-lock", None, npm_hidden_lock, 0o644)]
    total = len(npm_hidden_lock)
    for location, archive in ((SDK_LOCATION, package.archive),
                              *((row.dependency.location, row.archive) for row in dependencies)):
        for member in archive.members:
            _require(len(rows) < MAX_INSTALLED_FILES, "installed regular-file count exceeds bound")
            _require(len(member.content) <= MAX_MEMBER_BYTES
                     and total + len(member.content) <= MAX_INSTALLED_BYTES,
                     "installed content exceeds byte bound")
            name = location.removeprefix("node_modules/") + "/" + member.name
            _require(len(name.split("/")) <= MAX_DIRECTORY_DEPTH, "installed path exceeds directory depth")
            inventory.admit(name)
            rows.append(InstalledMember(name, location, member.name, member.content, member.mode))
            total += len(member.content)
    return InstalledProjection(package_raw, tuple(sorted(sources.items())), checksum_manifest,
                               lock, tuple(sorted(dependency_originals.items())), npm_hidden_lock,
                               environment_label, sdk_archive_label, tuple(sorted(rows, key=lambda row: row.path)))


def validate_projection(projection: InstalledProjection) -> None:
    """Rederive all fields from retained original bytes, rejecting forged views.

    This validates a content relation only. Resealing different source/original
    bytes cannot establish that the caller independently authenticated them.
    """
    _require(type(projection) is InstalledProjection, "installed projection has a foreign type")
    for rows, bound in ((projection.sources, MAX_SOURCE_FILES), (projection.dependency_originals, 9)):
        _require(type(rows) is tuple and 0 < len(rows) <= bound
                 and all(type(row) is tuple and len(row) == 2 and type(row[0]) is str
                         and 0 < len(row[0]) <= MAX_NAME_BYTES
                         and type(row[1]) is bytes for row in rows), "installed original inventory shape")
        names = [row[0] for row in rows]
        _require(names == sorted(set(names)), "installed original names duplicate or are not canonical")
    _require(type(projection.members) is tuple and 0 < len(projection.members) <= MAX_INSTALLED_FILES
             and all(type(row) is InstalledMember and type(row.path) is str and type(row.owner) is str
                     and (row.archive_member is None or type(row.archive_member) is str)
                     and type(row.content) is bytes and type(row.mode) is int
                     for row in projection.members),
             "installed member inventory shape")
    expected = installed_projection(projection.package_raw, sources=dict(projection.sources),
                                    checksum_manifest=projection.checksum_manifest, lock=projection.lock,
                                    dependency_originals=dict(projection.dependency_originals),
                                    npm_hidden_lock=projection.npm_hidden_lock,
                                    environment_label=projection.environment_label,
                                    sdk_archive_label=projection.sdk_archive_label)
    _require(expected == projection, "installed projection differs from its exact original content")


def verify_installed_content(projection: InstalledProjection, *, files: dict[str, bytes],
                             modes: dict[str, int]) -> None:
    """Compare captured installed bytes and modes, with no filesystem authority."""
    validate_projection(projection)
    _require(type(files) is dict and type(modes) is dict
             and len(files) == len(modes) == len(projection.members)
             and all(type(name) is str and 0 < len(name) <= MAX_NAME_BYTES for name in files)
             and all(type(name) is str and 0 < len(name) <= MAX_NAME_BYTES for name in modes)
             and set(files) == set(modes) == {row.path for row in projection.members},
             "installed inventory differs from the exact original projection")
    for row in projection.members:
        _require(type(files[row.path]) is bytes and files[row.path] == row.content
                 and type(modes[row.path]) is int and modes[row.path] == row.mode,
                 "installed file bytes or mode differ from original archive")
