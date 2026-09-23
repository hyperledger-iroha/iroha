"""Bounded original-byte Darwin arm64 Node24 manifest/bundle relation.

No filesystem I/O, runtime discovery, process, native loading, or approval token.
The caller supplies an independent manifest digest; a match does not establish
who approved it. Version is an input expectation pending an actual process join.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from pathlib import PurePosixPath
import re
import struct

from sorafs_evidence_json import decode_evidence_json
from sorafs_python_consumer_artifact import canonical_json
from sorafs_javascript_runtime_graph import (
    MAX_IMAGE_BYTES, MAX_IMAGE_LOADS, MAX_INHERITED_RPATHS, MAX_RPATHS, RuntimeInputError,
    absolute_path, normal_os_path,
    inherited_rpath_states, project_node_image, require, text_path,
)

SCHEMA = "sorafs.javascript.runtime_inputs.v1"
MAGIC = b"SORAFS_JAVASCRIPT_RUNTIME_INPUTS_V1\n"
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_IMAGES = 128
MAX_ALIASES = 256
MAX_EDGES = 4096
MAX_CANDIDATES = 16384
MAX_RUNTIME_BYTES = 512 * 1024 * 1024
MAX_BUNDLE_BYTES = len(MAGIC) + 8 + MAX_MANIFEST_BYTES + MAX_RUNTIME_BYTES
MAX_NAMESPACE_NODES = 32768
MAX_NAMESPACE_BYTES = 4 * 1024 * 1024


def _closed(value: object, fields: set[str], label: str) -> dict:
    require(type(value) is dict and set(value) == fields, label + " fields differ")
    return value


def _integer(value: object, minimum: int, maximum: int, label: str) -> int:
    require(type(value) is int and minimum <= value <= maximum, label + " integer bound")
    return value


def _digest(value: object) -> str:
    require(type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value) is not None
            and value != "0" * 64, "runtime digest differs")
    return value


def _rows(value: object, maximum: int, label: str) -> list:
    require(type(value) is list and len(value) <= maximum, label + " count bound")
    return value


@dataclass(frozen=True)
class RuntimeImage:
    """One original regular image's claimed bytes and permission mode."""
    path: str
    sha256: str
    size: int
    mode: int


@dataclass(frozen=True)
class RuntimeAlias:
    """An explicit original symlink target and its derived final pathname."""
    path: str
    target: str
    resolved: str


@dataclass(frozen=True)
class RuntimeCandidate:
    """One ordered candidate; None means an absent leaf, never a dangling link."""
    path: str
    resolved: str | None


@dataclass(frozen=True)
class RuntimeEdge:
    """One original command and every candidate in its supported search profile."""
    source: str
    index: int
    command: int
    name: str
    scope: str
    candidates: tuple[RuntimeCandidate, ...]
    selected: str | None


@dataclass(frozen=True)
class NodeRuntimeManifest:
    """Strict pinned input claims, pending original-byte and physical joins."""
    raw: bytes
    sha256: str
    version: str
    selected_executable: str
    executable: str
    images: tuple[RuntimeImage, ...]
    aliases: tuple[RuntimeAlias, ...]
    edges: tuple[RuntimeEdge, ...]


class _Namespace:
    """A bounded pure original-path namespace, not a filesystem simulation."""
    def __init__(self, manifest: NodeRuntimeManifest, *, candidate_paths: tuple[str, ...] | None = None):
        self.files = {row.path for row in manifest.images}
        self.links = {row.path: row for row in manifest.aliases}
        self.directories = {"/"}
        self.used: set[str] = set()
        if candidate_paths is None:
            candidate_paths = tuple(slot.path for edge in manifest.edges for slot in edge.candidates)
        paths = (*self.files, *self.links, *(row.resolved for row in manifest.aliases),
                 manifest.selected_executable, *candidate_paths)
        spellings: dict[str, str] = {}
        size = 0
        for path in paths:
            for current in (path, *(str(parent) for parent in PurePosixPath(path).parents)):
                key = current.casefold()
                require(key not in spellings or spellings[key] == current,
                        "runtime namespace case alias")
                if key not in spellings:
                    encoded = len(current.encode("utf-8"))
                    require(len(spellings) < MAX_NAMESPACE_NODES
                            and size + encoded <= MAX_NAMESPACE_BYTES,
                            "runtime namespace allocation bound")
                    spellings[key] = current
                    size += encoded
            self.directories.update(str(parent) for parent in PurePosixPath(path).parents)
        require(not self.files.intersection(self.links)
                and not self.files.intersection(self.directories), "runtime file/ancestor alias")
        # Canonical originals may never sit beneath a declared symlink.
        require(not any(str(parent) in self.links for file in self.files
                        for parent in PurePosixPath(file).parents),
                "runtime original path has a symlink ancestor")

    def resolve(self, path: str, *, absent_leaf: bool = False) -> str | None:
        pending = path.split("/")[1:]
        resolved: list[str] = []
        followed = 0
        leaf_alias = False
        while pending:
            part = pending.pop(0)
            if part in ("", "."):
                continue
            if part == "..":
                require(bool(resolved), "runtime alias escapes root")
                resolved.pop()
                continue
            current = "/" + "/".join((*resolved, part))
            if current in self.links:
                followed += 1
                require(followed <= MAX_ALIASES, "runtime alias cycle/depth")
                self.used.add(current)
                leaf_alias = leaf_alias or not pending
                target = self.links[current].target
                if target.startswith("/"):
                    resolved = []
                pending = target.split("/") + pending
            else:
                if not pending and current not in self.files and current not in self.directories:
                    require(absent_leaf and not leaf_alias, "runtime alias target is missing")
                    return None
                require(current in self.directories if pending else
                        current in self.files or current in self.directories,
                        "runtime alias crosses a non-directory")
                resolved.append(part)
        return "/" + "/".join(resolved)


def parse_node_runtime_manifest(raw: bytes, *, expected_sha256: str) -> NodeRuntimeManifest:
    """Validate the independently pinned, closed V1 input schema before byte joins."""
    _digest(expected_sha256)
    require(type(raw) is bytes and 0 < len(raw) <= MAX_MANIFEST_BYTES, "runtime manifest byte bound")
    require(hashlib.sha256(raw).hexdigest() == expected_sha256, "independent runtime manifest pin differs")
    try:
        row = decode_evidence_json(raw)
        require(canonical_json(row) == raw, "runtime manifest JSON is not canonical")
    except (ValueError, UnicodeError, RecursionError) as error:
        raise RuntimeInputError("invalid runtime manifest JSON") from error
    _closed(row, {"schema", "platform", "architecture", "version", "selected_executable",
                  "executable", "images", "aliases", "edges"}, "runtime manifest")
    require((row["schema"], row["platform"], row["architecture"]) == (SCHEMA, "darwin", "arm64"),
            "unsupported runtime profile")
    require(type(row["version"]) is str
            and re.fullmatch(r"24\.(?:0|[1-9][0-9]{0,4})\.(?:0|[1-9][0-9]{0,4})", row["version"]) is not None,
            "runtime version must be exact stable Node24")
    executable, selected = absolute_path(row["executable"]), absolute_path(row["selected_executable"])
    images = []
    total = 0
    for value in _rows(row["images"], MAX_IMAGES, "runtime images"):
        value = _closed(value, {"path", "sha256", "size", "mode"}, "runtime image")
        path = absolute_path(value["path"])
        require(not normal_os_path(path), "normal-OS image cannot be a captured runtime original")
        size = _integer(value["size"], 32, MAX_IMAGE_BYTES, "runtime image size")
        total += size
        require(total <= MAX_RUNTIME_BYTES, "runtime aggregate image byte bound")
        mode = _integer(value["mode"], 0, 0o7777, "runtime image mode")
        require(mode in (0o444, 0o555, 0o644, 0o755) and (path != executable or mode & 0o100),
                "runtime image mode differs")
        images.append(RuntimeImage(path, _digest(value["sha256"]), size, mode))
    names = tuple(value.path for value in images)
    require(names and names == tuple(sorted(set(names))) and executable in names,
            "runtime images must be sorted/unique and contain the executable")
    aliases = []
    for value in _rows(row["aliases"], MAX_ALIASES, "runtime aliases"):
        value = _closed(value, {"path", "target", "resolved"}, "runtime alias")
        path, target, resolved = absolute_path(value["path"]), text_path(value["target"]), absolute_path(value["resolved"])
        require(not normal_os_path(path) and not normal_os_path(resolved), "runtime alias crosses normal-OS boundary")
        require(not target.startswith("//") and all(part not in ("", ".") for part in target.lstrip("/").split("/")),
                "runtime alias target spelling differs")
        aliases.append(RuntimeAlias(path, target, resolved))
    require(tuple(value.path for value in aliases) == tuple(sorted({value.path for value in aliases})),
            "runtime aliases must be sorted/unique")
    edges = []
    candidate_count = 0
    for value in _rows(row["edges"], MAX_EDGES, "runtime edges"):
        value = _closed(value, {"source", "index", "command", "name", "scope", "candidates", "selected"}, "runtime edge")
        source = absolute_path(value["source"])
        require(source in names, "runtime edge source is not an original")
        index = _integer(value["index"], 0, 4095, "runtime command index")
        require(type(value["command"]) is int and value["command"] == 0xC, "runtime load command differs")
        require(value["scope"] in ("normal_os", "runtime"), "runtime edge scope differs")
        candidates = []
        for slot in _rows(value["candidates"], 32, "runtime edge candidates"):
            slot = _closed(slot, {"path", "resolved"}, "runtime candidate")
            candidate_count += 1
            require(candidate_count <= MAX_CANDIDATES, "runtime aggregate candidate bound")
            path = absolute_path(slot["path"])
            target = None if slot["resolved"] is None else absolute_path(slot["resolved"])
            require(not normal_os_path(path) and (target is None or target in names), "runtime candidate ownership differs")
            candidates.append(RuntimeCandidate(path, target))
        target = None if value["selected"] is None else absolute_path(value["selected"])
        require(target is None or target in names, "runtime selected image is absent")
        edges.append(RuntimeEdge(source, index, value["command"], text_path(value["name"]),
                                 value["scope"], tuple(candidates), target))
    keys = tuple((edge.source, edge.index) for edge in edges)
    require(keys == tuple(sorted(set(keys))), "runtime edges must retain exact source/command order")
    manifest = NodeRuntimeManifest(raw, expected_sha256, row["version"], selected,
                                   executable, tuple(images), tuple(aliases), tuple(edges))
    namespace = _Namespace(manifest)
    for alias in aliases:
        require(namespace.resolve(alias.path) == alias.resolved, "runtime alias claimed resolution differs")
    require(namespace.resolve(selected) == executable, "selected runtime executable does not resolve to original")
    return manifest


@dataclass(frozen=True)
class NodeRuntimeBundle:
    """Original bytes and rederived graph, without physical/runtime authority."""
    raw: bytes
    manifest: NodeRuntimeManifest
    offsets: tuple[tuple[str, int, int], ...]

    def member_bytes(self, path: str) -> bytes:
        """Return one already-validated original member, never open its old path."""
        for member, offset, size in self.offsets:
            if member == path:
                return self.raw[offset:offset + size]
        raise RuntimeInputError("runtime bundle member is absent")


def parse_node_runtime_bundle(raw: bytes, *, expected_manifest_sha256: str) -> NodeRuntimeBundle:
    """Join exact framed originals, command graph, aliases, candidates and EOF."""
    header = len(MAGIC) + 8
    require(type(raw) is bytes and header < len(raw) <= MAX_BUNDLE_BYTES and raw.startswith(MAGIC),
            "runtime bundle envelope differs")
    length = struct.unpack_from(">Q", raw, len(MAGIC))[0]
    require(0 < length <= MAX_MANIFEST_BYTES and header + length <= len(raw), "runtime manifest length bound")
    manifest = parse_node_runtime_manifest(raw[header:header + length], expected_sha256=expected_manifest_sha256)
    offset = header + length
    require(offset + sum(row.size for row in manifest.images) == len(raw), "runtime bundle missing/trailing bytes")
    offsets, projections = [], []
    remaining_edges, remaining_candidates = MAX_EDGES, MAX_CANDIDATES
    for row in manifest.images:
        require(hashlib.sha256(memoryview(raw)[offset:offset + row.size]).hexdigest() == row.sha256,
                "runtime original image digest differs")
        projection = project_node_image(raw, offset=offset, size=row.size, path=row.path, executable=manifest.executable,
                                        load_limit=min(MAX_IMAGE_LOADS, remaining_edges),
                                        candidate_limit=min(MAX_IMAGE_LOADS * MAX_RPATHS, remaining_candidates))
        remaining_edges -= len(projection.loads)
        remaining_candidates -= sum(len(load.candidates) for load in projection.loads)
        projections.append(projection)
        offsets.append((row.path, offset, row.size))
        offset += row.size
    namespace = _Namespace(manifest)
    require(namespace.resolve(manifest.selected_executable) == manifest.executable, "runtime executable relation differs")
    derived = []
    adjacency = {row.path: set() for row in manifest.images}
    ids = [projection.install_id for projection in projections if projection.install_id is not None]
    require(len(set(ids)) == len(ids), "runtime image install IDs collide")
    deferred = []
    for projection in projections:
        if projection.install_id is not None:
            require(namespace.resolve(projection.install_id) == projection.path,
                    "runtime install ID resolves to a different original")
        for load in projection.loads:
            if projection.path != manifest.executable and load.name.startswith("@rpath/"):
                deferred.append((projection, load))
                continue
            candidates = tuple(RuntimeCandidate(path, namespace.resolve(path, absent_leaf=True))
                               for path in load.candidates)
            present = {slot.resolved for slot in candidates if slot.resolved is not None}
            if load.normal_os:
                selected = None
            else:
                require(len(present) == 1 and present <= namespace.files,
                        "runtime candidates must select one exact original without ambiguity")
                selected = next(iter(present))
                adjacency[projection.path].add(selected)
            derived.append(RuntimeEdge(projection.path, load.index, load.command, load.name,
                                       "normal_os" if load.normal_os else "runtime", candidates, selected))
    actual_candidates = sum(len(edge.candidates) for edge in derived)
    if deferred:
        ancestry = inherited_rpath_states({row.path: row for row in projections}, adjacency,
                                           manifest.executable)
        for projection, load in deferred:
            routes = ancestry[projection.path]
            require(len(routes) == 1, "runtime shared rpath has unreachable or differing ancestry")
            bases = next(iter(routes))
            require(bases, "runtime shared rpath has no candidate bases")
            tail = load.name[len("@rpath/"):]
            require(len(bases) <= MAX_INHERITED_RPATHS
                    and actual_candidates + len(bases) <= MAX_CANDIDATES,
                    "runtime shared rpath candidate admission bound")
            actual_candidates += len(bases)
            candidates = tuple(RuntimeCandidate(absolute_path(base + "/" + tail),
                                                namespace.resolve(absolute_path(base + "/" + tail),
                                                                  absent_leaf=True))
                               for base in bases)
            present = {slot.resolved for slot in candidates if slot.resolved is not None}
            require(len(present) == 1 and present <= namespace.files,
                    "runtime inherited candidates must select one exact original without ambiguity")
            selected = next(iter(present))
            adjacency[projection.path].add(selected)
            derived.append(RuntimeEdge(projection.path, load.index, load.command, load.name,
                                       "runtime", candidates, selected))
    derived.sort(key=lambda edge: (edge.source, edge.index))
    cached_names: dict[str, str | None] = {}
    for edge in derived:
        if edge.scope == "runtime" and edge.name.startswith("@rpath/"):
            previous = cached_names.setdefault(edge.name, edge.selected)
            require(previous == edge.selected,
                    "runtime shared rpath cached-name target is ambiguous")
    require(tuple(derived) == manifest.edges, "runtime original command/candidate relation differs")
    require(namespace.used == set(namespace.links), "runtime alias inventory contains unused originals")
    reached, pending = set(), [manifest.executable]
    while pending:
        current = pending.pop()
        if current not in reached:
            reached.add(current)
            pending.extend(adjacency[current] - reached)
    require(reached == namespace.files, "runtime contains unreachable or missing original images")
    return NodeRuntimeBundle(raw, manifest, tuple(offsets))
