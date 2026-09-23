"""Pure, narrow Darwin arm64 image projection using the sole Mach-O decoder.

This is an input relation, not dyld emulation or execution authority. Only thin
arm64 images and bounded, direct-edge ancestry are projected. Weak/reexport/lazy
loads, environment commands and unknown commands are refused.
"""
from __future__ import annotations

from dataclasses import dataclass
import posixpath
import struct
import unicodedata
from collections import deque

from copy_sumeragi_v2_release_cargo_cache_cli import (
    _MachOError, _parse_macho_thin, _macho_c_string,
)

MAX_IMAGE_BYTES = 256 * 1024 * 1024
MAX_COMMAND_BYTES = 1024 * 1024
MAX_COMMANDS = 4096
MAX_IMAGE_LOADS = 256
MAX_RPATHS = 32
MAX_PATH_BYTES = 4096
MAX_ANCESTRY_STATES = 4096
MAX_ANCESTRY_BYTES = 4 * 1024 * 1024
MAX_INHERITED_RPATHS = 32
# The accepted stock arm64 profile has no other loader-affecting command.
COMMANDS = frozenset({0x19, 0x1B, 0x1D, 0x2, 0x26, 0x29, 0x2A, 0x32,
                     0x8000001C, 0x80000028, 0x80000033, 0x80000034,
                     0xB, 0xC, 0xD, 0xE})


class RuntimeInputError(ValueError):
    """The captured runtime input does not satisfy this fixed profile."""


def require(condition: bool, message: str) -> None:
    """Refuse an invalid external input without conferring runtime authority."""
    if not condition:
        raise RuntimeInputError(message)


def text_path(value: object) -> str:
    """Bound UTF-8 names and refuse control/normalization ambiguities."""
    require(type(value) is str and 0 < len(value) <= MAX_PATH_BYTES,
            "runtime path character bound")
    try:
        size = len(value.encode("utf-8"))
    except UnicodeEncodeError as error:
        raise RuntimeInputError("runtime path encoding differs") from error
    require(0 < size <= MAX_PATH_BYTES and "\\" not in value
            and not any(ord(char) < 32 or ord(char) == 127 for char in value)
            and unicodedata.normalize("NFC", value) == value, "runtime path is not canonical")
    return value


def absolute_path(value: object) -> str:
    """Accept one exact non-root POSIX path, with no lexical aliases."""
    value = text_path(value)
    require(value.startswith("/") and not value.startswith("//") and value != "/"
            and len(value[1:].split("/")) <= 64
            and all(part not in ("", ".", "..") and len(part.encode("utf-8")) <= 255
                    for part in value[1:].split("/")),
            "runtime path must be canonical absolute POSIX")
    return value


def normal_os_path(value: str) -> bool:
    """Classify only canonical, direct paths inside the fixed normal-OS boundary."""
    absolute_path(value)
    for boundary in ("/usr/lib", "/System/Library"):
        folded = value.casefold()
        if folded == boundary.casefold() or folded.startswith(boundary.casefold() + "/"):
            require(value.startswith(boundary + "/"),
                    "runtime normal-OS boundary spelling/directory differs")
            return True
    return False


def _expanded(value: str, source: str, executable: str) -> str:
    text_path(value)
    if value.startswith("/"):
        return absolute_path(value)
    for token, base in (("@loader_path", posixpath.dirname(source)),
                        ("@executable_path", posixpath.dirname(executable))):
        if value == token:
            return absolute_path(base)
        if value.startswith(token + "/"):
            parts = value[len(token) + 1:].split("/")
            # Leading .. applies to a canonical original directory. Internal ..
            # could cross a symlink, which requires a broader loader profile.
            while parts and parts[0] == "..":
                require(base != "/", "runtime token escapes root")
                base = posixpath.dirname(base)
                parts.pop(0)
            require(parts and all(part not in ("", ".", "..") for part in parts),
                    "runtime token tail is ambiguous")
            return absolute_path(base.rstrip("/") + "/" + "/".join(parts))
    raise RuntimeInputError("unsupported runtime loader token")


def inherited_rpath_states(projections: dict[str, ImageProjection],
                           direct_edges: dict[str, set[str]], executable: str,
                           *, state_limit: int = MAX_ANCESTRY_STATES,
                           byte_limit: int = MAX_ANCESTRY_BYTES
                           ) -> dict[str, frozenset[tuple[str, ...]]]:
    """Bound distinct original rpath sequences along direct dependency ancestry.

    A shared-rpath edge cannot establish its own ancestry. Only independently
    resolved direct edges contribute; an unreachable source or differing route
    sequence is refused by the caller. These are candidate slots, not a claim
    about dyld loadability, cache selection or the actual mapped image.
    """
    executable = absolute_path(executable)
    require(type(state_limit) is int and 0 < state_limit <= MAX_ANCESTRY_STATES
            and type(byte_limit) is int and 0 < byte_limit <= MAX_ANCESTRY_BYTES,
            "runtime ancestry admission bound")
    require(executable in projections and set(projections) == set(direct_edges),
            "runtime ancestry graph ownership differs")
    indegree = {path: 0 for path in projections}
    for children in direct_edges.values():
        for child in children:
            require(child in indegree, "runtime ancestry edge is not an original")
            indegree[child] += 1
    ready = deque(sorted(path for path, count in indegree.items() if count == 0))
    states: dict[str, set[tuple[str, ...]]] = {path: set() for path in projections}
    count = used = 0

    def admit(path: str, inherited: tuple[str, ...]) -> None:
        nonlocal count, used
        bases = tuple(_expanded(value, path, executable) for value in projections[path].rpaths)
        sequence = bases + inherited
        require(len(sequence) <= MAX_INHERITED_RPATHS,
                "runtime inherited rpath slot bound")
        if sequence in states[path]:
            return
        charge = sum(len(value.encode("utf-8")) for value in sequence)
        require(count < state_limit and used + charge <= byte_limit,
                "runtime ancestry state admission bound")
        states[path].add(sequence)
        count += 1
        used += charge

    admit(executable, ())
    visited = 0
    while ready:
        parent = ready.popleft()
        visited += 1
        for child in sorted(direct_edges[parent]):
            for sequence in states[parent]:
                admit(child, sequence)
            indegree[child] -= 1
            if indegree[child] == 0:
                ready.append(child)
    require(visited == len(projections), "runtime direct ancestry is cyclic")
    return {path: frozenset(sequences) for path, sequences in states.items()}


@dataclass(frozen=True)
class ImageLoad:
    """An ordered original load command and its derived search candidates."""
    index: int
    command: int
    name: str
    candidates: tuple[str, ...]
    normal_os: bool


@dataclass(frozen=True)
class ImageProjection:
    """Decoded input facts, with no physical or execution authority."""
    path: str
    install_id: str | None
    rpaths: tuple[str, ...]
    loads: tuple[ImageLoad, ...]


def project_node_image(raw: bytes, *, offset: int, size: int,
                       path: str, executable: str, load_limit: int = MAX_IMAGE_LOADS,
                       candidate_limit: int = MAX_IMAGE_LOADS * MAX_RPATHS) -> ImageProjection:
    """Project a bounded original slice without copying the full bundled image.

    The initial fixed-header admission bounds allocations in the shared parser;
    the shared parser remains the sole load-command/string/extent decoder.
    """
    path, executable = absolute_path(path), absolute_path(executable)
    require(type(load_limit) is int and 0 <= load_limit <= MAX_IMAGE_LOADS
            and type(candidate_limit) is int and 0 <= candidate_limit <= MAX_IMAGE_LOADS * MAX_RPATHS,
            "runtime projection budget differs")
    require(type(raw) is bytes and type(offset) is int and type(size) is int
            and offset >= 0 and 32 <= size <= MAX_IMAGE_BYTES
            and offset + size <= len(raw), "runtime image extent bound")
    require(raw[offset:offset + 4] == b"\xcf\xfa\xed\xfe",
            "runtime requires a thin little-endian arm64 image; fat images unsupported")
    header = struct.unpack_from("<8I", raw, offset)
    require(header[1:4] == (0x0100000C, 0, 2 if path == executable else 6)
            and header[7] == 0, "runtime Mach-O architecture/type/reserved differs")
    require(0 < header[4] <= MAX_COMMANDS and 0 < header[5] <= MAX_COMMAND_BYTES
            and 32 + header[5] <= size, "runtime command table admission bound")
    try:
        image = _parse_macho_thin(raw, offset, size, path)
        commands = image["commands"]
        require(all(row["command"] in COMMANDS for row in commands),
                "runtime load command is outside the closed profile")
        rpaths = tuple(row["name"] for row in commands if row["command"] == 0x8000001C)
        require(len(rpaths) <= MAX_RPATHS and len(set(rpaths)) == len(rpaths),
                "runtime rpath inventory bound/duplicate")
        bases = tuple(_expanded(value, path, executable) for value in rpaths)
        require(len(set(bases)) == len(bases), "runtime rpath candidate aliases")
        ids = tuple(row["name"] for row in commands if row["command"] == 0xD)
        require(len(ids) == (0 if path == executable else 1), "runtime install ID count differs")
        for value in ids:
            require(not normal_os_path(absolute_path(value)), "runtime install ID must name a captured original")
        linkers = tuple(row for row in commands if row["command"] == 0xE)
        require(len(linkers) == (1 if path == executable else 0), "runtime dynamic linker count differs")
        for row in linkers:
            require(row["size"] >= 12, "runtime dynamic linker command is truncated")
            name_offset = struct.unpack_from("<I", raw, row["offset"] + 8)[0]
            require(12 <= name_offset < row["size"], "runtime dynamic linker string extent")
            name = _macho_c_string(raw, row["offset"] + name_offset,
                                   row["offset"] + row["size"], path)
            require(name == "/usr/lib/dyld", "runtime dynamic linker differs")
        loads = []
        candidate_count = 0
        for row in commands:
            if row["command"] != 0xC:
                continue
            require(len(loads) < load_limit, "runtime image/aggregate load count bound")
            name = text_path(row["name"])
            system = name.startswith("/") and normal_os_path(name)
            if system:
                candidates = ()
            elif name.startswith("@rpath/"):
                require(path != executable or bases, "runtime executable rpath inventory is empty")
                tail = name[len("@rpath/"):]
                require(tail and all(part not in ("", ".", "..") for part in tail.split("/")),
                        "runtime rpath suffix is ambiguous")
                require(candidate_count + len(bases) <= candidate_limit, "runtime aggregate candidate admission bound")
                candidates = tuple(absolute_path(base + "/" + tail) for base in bases)
            else:
                require(candidate_count < candidate_limit, "runtime aggregate candidate admission bound")
                candidates = (_expanded(name, path, executable),)
            require(not any(normal_os_path(candidate) for candidate in candidates),
                    "token-expanded normal-OS lookup is unsupported")
            candidate_count += len(candidates)
            loads.append(ImageLoad(row["index"], row["command"], name, candidates, system))
    except _MachOError as error:
        raise RuntimeInputError("invalid runtime Mach-O input: " + str(error)) from error
    return ImageProjection(path, ids[0] if ids else None, rpaths, tuple(loads))
