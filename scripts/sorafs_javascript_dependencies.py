"""Original npm bytes for the fixed SoraFS JavaScript production dependency tree.

Dependency-free library: no downloads, install scripts, imports or filesystem
access. The caller supplies the independently selected candidate lock digest and
nine original tarballs. This binds content and placement, not npm execution,
loaded modules, platform support or release approval. Updating the production
closure requires reviewing this profile alongside the canonical package lock.
"""
from __future__ import annotations

import base64
from dataclasses import dataclass
import hashlib
import re

from sorafs_evidence_json import decode_evidence_json
from sorafs_javascript_archive import ArchiveError, NpmArchive, parse_npm_archive, _require

MAX_LOCK_BYTES = 1024 * 1024
MAX_PACKAGE_JSON_BYTES = 128 * 1024
MAX_ARCHIVES_BYTES = 64 * 1024 * 1024
MAX_TARS_BYTES = 128 * 1024 * 1024
VERSIONS = {
    "node_modules/@noble/ciphers": "1.3.0",
    "node_modules/@noble/curves": "1.9.7",
    "node_modules/@noble/hashes": "1.8.0",
    "node_modules/@scure/base": "2.2.0",
    "node_modules/@scure/bip39": "2.2.0",
    "node_modules/@scure/bip39/node_modules/@noble/hashes": "2.2.0",
    "node_modules/base64-js": "1.5.1",
    "node_modules/buffer": "6.0.3",
    "node_modules/ieee754": "1.2.1",
}
ROOT_DEPENDENCIES = {"@noble/ciphers": "^1.2.0", "@noble/curves": "^1.4.0",
                     "@noble/hashes": "^1.5.0", "@scure/bip39": "^2.2.0", "buffer": "^6.0.3"}
DEPENDENCIES = {
    "node_modules/@noble/curves": {"@noble/hashes": "1.8.0"},
    "node_modules/@scure/bip39": {"@noble/hashes": "2.2.0", "@scure/base": "2.2.0"},
    "node_modules/buffer": {"base64-js": "^1.3.1", "ieee754": "^1.2.1"},
}
_LOCK_FIELDS = {"version", "resolved", "integrity", "license", "engines", "funding", "dependencies"}
_INSTALL_FIELDS = {"optionalDependencies", "peerDependencies", "peerDependenciesMeta",
                   "bundleDependencies", "bundledDependencies", "workspaces", "bin", "gypfile",
                   "directories", "man", "acceptDependencies"}
_PACKAGE_FIELDS = {"author", "browser", "bugs", "contributors", "dependencies", "description",
                   "devDependencies", "engines", "exports", "files", "funding", "homepage", "jspm",
                   "keywords", "license", "main", "module", "name", "repository", "scripts",
                   "sideEffects", "standard", "type", "types", "typings", "version"}
_INSTALL_SCRIPTS = {"preinstall", "install", "postinstall", "prepare", "prepublish"}


def _json(raw: bytes, limit: int) -> dict:
    _require(type(raw) is bytes and 0 < len(raw) <= limit, "npm JSON byte bound")
    try:
        return decode_evidence_json(raw)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise ArchiveError("npm metadata is not one bounded duplicate-free JSON object") from error


@dataclass(frozen=True)
class Dependency:
    """An original registry identity at one exact resolution location."""
    location: str
    name: str
    version: str
    resolved: str
    integrity: str
    dependencies: tuple[tuple[str, str], ...]
    engines: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class DependencyLock:
    """Captured candidate lock bytes and the sole admitted production graph."""
    raw: bytes
    sha256: str
    sdk_version: str
    dependencies: tuple[Dependency, ...]


@dataclass(frozen=True)
class DependencyContent:
    """Original content joined to its exact lock-owned install location."""
    dependency: Dependency
    archive: NpmArchive


def parse_dependency_lock(raw: bytes, *, expected_sha256: str) -> DependencyLock:
    """Read the current nine-location graph with an independently selected pin."""
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_LOCK_BYTES,
             "npm lock byte bound")
    _require(type(expected_sha256) is str and re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is not None
             and hashlib.sha256(raw).hexdigest() == expected_sha256,
             "npm lock differs from the independently selected candidate")
    lock = _json(raw, MAX_LOCK_BYTES)
    _require(set(lock) == {"name", "version", "lockfileVersion", "requires", "packages"}
             and lock["name"] == "@iroha/iroha-js" and type(lock["lockfileVersion"]) is int
             and lock["lockfileVersion"] == 3 and lock["requires"] is True,
             "npm lock is not the sole candidate V3 layout")
    version = lock["version"]
    _require(type(version) is str and len(version) <= 64
             and re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", version) is not None,
             "npm SDK version is not an exact stable release")
    packages = lock["packages"]
    _require(type(packages) is dict and 1 < len(packages) <= 512 and "" in packages
             and all(type(row) is dict and type(row.get("dev", False)) is bool for row in packages.values()),
             "npm package lock inventory is malformed or unbounded")
    root = packages[""]
    _require(set(root) <= {"name", "version", "license", "dependencies", "devDependencies", "engines"}
             and root.get("name") == lock["name"] and root.get("version") == version
             and root.get("dependencies") == ROOT_DEPENDENCIES,
             "npm root dependency selection differs from the fixed profile")
    production = {path: row for path, row in packages.items() if path and not row.get("dev", False)}
    _require(set(production) == set(VERSIONS), "npm production dependency placement differs")
    dependencies = []
    for location, selected_version in VERSIONS.items():
        row = production[location]
        _require(set(row) <= _LOCK_FIELDS and {"version", "resolved", "integrity"} <= set(row)
                 and row["version"] == selected_version,
                 "npm dependency introduces an unreviewed version or install policy")
        name = location.rsplit("node_modules/", 1)[1]
        resolved = f"https://registry.npmjs.org/{name}/-/{name.rsplit('/', 1)[-1]}-{selected_version}.tgz"
        _require(row["resolved"] == resolved, "npm dependency has a foreign registry or file source")
        integrity = row["integrity"]
        _require(type(integrity) is str and re.fullmatch(r"sha512-[A-Za-z0-9+/]{86}==", integrity) is not None,
                 "npm dependency must have exactly one SHA-512 integrity value")
        digest = base64.b64decode(integrity[7:], validate=True)
        _require(base64.b64encode(digest).decode("ascii") == integrity[7:], "npm integrity is not canonical base64")
        declared = row.get("dependencies", {})
        _require(declared == DEPENDENCIES.get(location, {}), "npm dependency resolution edges differ")
        engines = row.get("engines", {})
        _require(type(engines) is dict and set(engines) <= {"node"}
                 and all(type(value) is str and 0 < len(value) <= 128 for value in engines.values()),
                 "npm dependency engine declaration is malformed")
        dependencies.append(Dependency(location, name, selected_version, resolved, integrity,
                                       tuple(sorted(declared.items())), tuple(sorted(engines.items()))))
    return DependencyLock(raw, expected_sha256, version, tuple(dependencies))


def _package(archive: NpmArchive, dependency: Dependency) -> None:
    files = archive.files()
    _require("package.json" in files, "npm dependency omits package.json")
    row = _json(files["package.json"], MAX_PACKAGE_JSON_BYTES)
    _require(row.get("name") == dependency.name and row.get("version") == dependency.version
             and row.get("dependencies", {}) == dict(dependency.dependencies)
             and row.get("engines", {}) == dict(dependency.engines),
             "npm dependency metadata differs from its exact lock-owned identity")
    _require(not set(row) & _INSTALL_FIELDS, "npm dependency introduces install-time resolution or executables")
    _require(set(row) <= _PACKAGE_FIELDS, "npm dependency metadata exceeds the reviewed fixed profile")
    scripts = row.get("scripts", {})
    _require(type(scripts) is dict and not set(scripts) & _INSTALL_SCRIPTS,
             "npm dependency introduces an install lifecycle script")
    for name in files:
        parts = name.casefold().split("/")
        _require(not {"node_modules", ".npmrc", "npm-shrinkwrap.json", "package-lock.json"} & set(parts)
                 and not name.casefold().endswith((".node", ".wasm", ".wasi", ".dll", ".dylib", ".so", ".gyp")),
                 "npm dependency contains a bundled, startup, native or retired VM artifact")


def parse_dependency_archives(lock: DependencyLock, originals: dict[str, bytes]) -> tuple[DependencyContent, ...]:
    """Join nine original gzip files, fixed placement and all package members.

    Aggregate compressed and inflated budgets are admitted before allocations.
    The two @noble/hashes versions remain distinct owners; a name-keyed map or
    registry re-resolution cannot replace the selected nested installation.
    """
    _require(type(lock) is DependencyLock
             and parse_dependency_lock(lock.raw, expected_sha256=lock.sha256) == lock,
             "npm dependency projection differs from the pinned original lock")
    _require(type(originals) is dict and set(originals) == set(VERSIONS)
             and all(type(raw) is bytes and raw for raw in originals.values())
             and sum(map(len, originals.values())) <= MAX_ARCHIVES_BYTES,
             "npm dependency original inventory or aggregate compressed bound")
    result, total = [], 0
    for dependency in lock.dependencies:
        raw = originals[dependency.location]
        integrity = "sha512-" + base64.b64encode(hashlib.sha512(raw).digest()).decode("ascii")
        _require(integrity == dependency.integrity, "npm original tarball differs from the lock integrity")
        archive = parse_npm_archive(raw, tar_byte_limit=MAX_TARS_BYTES - total)
        total += archive.tar_size
        _package(archive, dependency)
        result.append(DependencyContent(dependency, archive))
    return tuple(result)
