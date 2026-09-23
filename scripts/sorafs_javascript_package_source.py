"""Join original npm package members to complete captured candidate source bytes.

Pure library: no filesystem access, extraction, build, npm or native execution.
The caller must authenticate a complete candidate source census and retain its
ownership through execution/publication. This content relation does not grant
that authority. Native checksum bytes stay a separate original input; their
governed provenance and actual loaded addon remain the native verifier's job.

TODO: consume this relation in the fixed installed-package producer and index
adapter with their actual complete source/native/runtime input owners.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import re

from sorafs_javascript_archive import (
    MAX_NAME_BYTES, ArchiveError, NpmArchive, NpmPathInventory, _path, _require, parse_npm_archive,
)
from sorafs_javascript_dependencies import DependencyLock, _json, parse_dependency_lock

MAX_SOURCE_FILES = 4096
MAX_SOURCE_FILE_BYTES = 16 * 1024 * 1024
MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_CHECKSUM_BYTES = 1024 * 1024
PACKAGE_RECIPE_SHA256 = "8fa7850c8d50aa3a187343be895f590fa64a2150b5d617a3b36ea1aeb43f7d85"
BUILD_RECIPE_SHA256 = "59955e518b5047a4887ff6507826f04f1af3f250352e3d9cfa11c47a513150c6"
ENGINE_RECIPE_SHA256 = "54fc313b8b4da1c4953a2c68bce5a357d8b52be39b35f3f2fab8ca6a2bf80550"
ENGINE_CONTRACT_SHA256 = "ffe14c863ca7c189dfea331fb1c832cd15092ce6369955f3af50021ab4446d8c"
CHECKSUM_MEMBER = "native/iroha_js_host.checksums.json"
BUILD_RECIPE = "scripts/build-dist.mjs"
ENGINE_RECIPE = "scripts/check-node-engine.mjs"
ENGINE_CONTRACT = "scripts/node-engine-contract.mjs"
IMPLICIT_MEMBERS = frozenset(("recipes/README.md",))
_SELECTORS = frozenset((".npmignore", ".gitignore", ".npmrc", "npm-shrinkwrap.json", "node_modules"))
# This is a fixed copy recipe, not an npm ignore/glob interpreter. The reviewed
# source layout has only these directories and ordinary module basenames.
_SOURCE_NAME = re.compile(r"src/(?:public/|kotodamaCompiler/)?[A-Za-z][A-Za-z0-9_-]*(?:\.browser)?(?:\.js|\.d\.ts)")
# Keep this relation with the pinned build-dist recipe: these outputs are
# mandatory even when a purported complete census omits the same package file.
REQUIRED_OUTPUTS = frozenset((
    "address.js", "atomicPrivateSettlement.js", "browser.js", "curveRegistry.js",
    "ivmArtifact.js", "kagemusha.js", "native.js", "nativeArtifactHash.js",
    "numericV1.js", "strictLosslessJson.js", "sorafsOrderbookSubmission.js",
    "sorafsOrderbookSubmission.d.ts", "smartContractDeploymentSubmit.js", "sumeragiTyped.js",
    "tairaTestnetProfile.js", "toriiBrowserClient.js", "toriiClient.js", "toriiOptional.js",
    "kotodamaCompiler/index.js", "kotodamaCompiler/browser.js", "kotodamaCompiler/client.js",
    "kotodamaCompiler/nativeBridge.js", "kotodamaCompiler/normalize.js",
))
CONSUMER_OUTPUTS = frozenset(("index.js", "native.js", "toriiTestHooks.js", "public/sorafs.js",
                              "public/norito.js", "toriiClient.js"))


@dataclass(frozen=True)
class SourceMember:
    """An exact inert package member and its source-relative content owner."""
    name: str
    source: str
    content: bytes


@dataclass(frozen=True)
class PackageProjection:
    """Expected portable package content; no live source custody is asserted."""
    version: str
    source_sha256: tuple[tuple[str, str], ...]
    members: tuple[SourceMember, ...]


@dataclass(frozen=True)
class PackageContent:
    """Original compressed package joined to its exact captured source projection."""
    archive: NpmArchive
    projection: PackageProjection


def package_projection(sources: dict[str, bytes], *, checksum_manifest: bytes,
                       lock: DependencyLock) -> PackageProjection:
    """Derive the one reviewed copy-to-dist recipe from a complete trusted census.

    Source names are SDK-root relative: all src/ regular files, published literal
    files, the implicit recipe README, package-lock.json, build-dist.mjs and
    check-node-engine.mjs and node-engine-contract.mjs. Unpublished code
    executed by prepack is mandatory.
    The native manifest is supplied separately, never selected from source/dist.
    No npm glob, ignore fallback or alternate compiler is reproduced here.
    """
    _require(type(sources) is dict and 0 < len(sources) <= MAX_SOURCE_FILES
             and all(type(raw) is bytes and len(raw) <= MAX_SOURCE_FILE_BYTES for raw in sources.values())
             and sum(map(len, sources.values())) <= MAX_SOURCE_BYTES,
             "JavaScript package source inventory or byte bound")
    for name in sources:
        _require(type(name) is str and len(name) <= MAX_NAME_BYTES - len("package/")
                 and _path("package/" + name) == name,
                 "JavaScript package source name differs from its canonical spelling")
        _require(not {part.casefold() for part in name.split("/")} & _SELECTORS,
                 "JavaScript package contains an unreviewed selection or npm configuration input")
    _require(type(lock) is DependencyLock
             and parse_dependency_lock(lock.raw, expected_sha256=lock.sha256) == lock
             and sources.get("package-lock.json") == lock.raw,
             "JavaScript package source lock differs from its original dependency owner")
    _require({"package.json", BUILD_RECIPE, ENGINE_RECIPE, ENGINE_CONTRACT} <= sources.keys()
             and hashlib.sha256(sources["package.json"]).hexdigest() == PACKAGE_RECIPE_SHA256
             and hashlib.sha256(sources[BUILD_RECIPE]).hexdigest() == BUILD_RECIPE_SHA256
             and hashlib.sha256(sources[ENGINE_RECIPE]).hexdigest() == ENGINE_RECIPE_SHA256
             and hashlib.sha256(sources[ENGINE_CONTRACT]).hexdigest() == ENGINE_CONTRACT_SHA256,
             "JavaScript package/build recipe differs from its reviewed exact selection")
    recipe = _json(sources["package.json"], 128 * 1024)
    _require(recipe["name"] == "@iroha/iroha-js" and recipe["version"] == lock.sdk_version,
             "JavaScript package identity differs from the candidate lock")
    literals = set(recipe["files"]) - {"dist", CHECKSUM_MEMBER}
    selected_sources = {name for name in sources if name.startswith("src/")}
    _require(selected_sources and all(_SOURCE_NAME.fullmatch(name) for name in selected_sources),
             "JavaScript source tree contains an unreviewed module path")
    _require(set(sources) == selected_sources | literals | IMPLICIT_MEMBERS
             | {BUILD_RECIPE, ENGINE_RECIPE, ENGINE_CONTRACT, "package-lock.json"},
             "JavaScript source census omits a required input or adds an unowned selection")
    _require({"src/" + name for name in REQUIRED_OUTPUTS | CONSUMER_OUTPUTS} <= selected_sources,
             "JavaScript source census omits a required build output or consumer entrypoint")
    _json(checksum_manifest, MAX_CHECKSUM_BYTES)
    rows = [SourceMember(name, name, sources[name]) for name in sorted(literals | IMPLICIT_MEMBERS)]
    rows.extend(SourceMember("dist/" + name.removeprefix("src/"), name, sources[name])
                for name in sorted(selected_sources))
    # Public package staging gives every member mode 0644. Original native and
    # checksum custody permissions remain unchanged; the .node is never packed.
    rows.append(SourceMember(CHECKSUM_MEMBER, "@original-native-checksum", checksum_manifest))
    paths = NpmPathInventory()
    for row in rows:
        paths.admit(row.name)
    return PackageProjection(lock.sdk_version,
                             tuple((name, hashlib.sha256(raw).hexdigest()) for name, raw in sorted(sources.items())),
                             tuple(sorted(rows, key=lambda row: row.name)))


def verify_package_content(raw: bytes, *, sources: dict[str, bytes], checksum_manifest: bytes,
                           lock: DependencyLock) -> PackageContent:
    """Compare actual bounded original tar members to every expected source byte.

    The returned projection identifies required source inputs but cannot prove
    the supplied census's completeness or native/SDK execution. Those are
    separate responsibilities of the original-source and executed-report owners.
    """
    projection = package_projection(sources, checksum_manifest=checksum_manifest, lock=lock)
    archive = parse_npm_archive(raw)
    expected = {row.name: row.content for row in projection.members}
    _require(archive.files() == expected and all(member.mode == 0o644 for member in archive.members),
             "JavaScript original package differs from exact source/member/permission projection")
    return PackageContent(archive, projection)
