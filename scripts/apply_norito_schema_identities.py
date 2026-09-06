#!/usr/bin/env python3
"""Generate one reviewed patch from compiler-captured Norito identity mappings.

Requires Python 3.10+ only; no environment variables, Cargo, capture, or source
writes. Run ``python3 scripts/apply_norito_schema_identities.py mapping.json``
to print a unified diff, or add ``--check`` to require that exact patch already
be present (exit 1 means pending edits; exit 2 means invalid input).

The sole input format is ``{"schema": 1, "files": [{"path": "relative.rs",
"sha256": "original file digest", "declarations": [{"start_byte": 123,
"end_byte": 147, "anchor": "pub struct Example(u32);", "kind": "struct",
"identifier": "Example", "nominal": "captured::Example"}]}]}``.
An optional ``frame`` string supplies the explicit root-frame projection.
Spans are UTF-8 byte offsets, from the visibility/item token through the final
semicolon or closing brace, excluding leading indentation and attributes.
Names are supplied declarations, never inferred; a generic nominal is the
reviewed constructor name consumed by the NoritoSchema derive.

This is the application step of a separate reviewed capture workflow, not a
compiler parser, resolver, inventory, or coverage proof. Only ordinary struct
and enum items at file/module scope with understood attributes are supported.
Macro-generated items, function-local declarations, unsupported attributes,
and generic frame projections require separate review. Existing identity
declarations require their original mapping for exact-result idempotence.
Visible wildcard imports and direct derive aliases are unresolved review items.
External semantic resolution (including reexports and implementations in other
files) remains the capture reviewer's responsibility and requires subsequent
compiler checks. Successful patch generation does not prove trait-impl uniqueness.

Every file and mapping is verified before any patch is printed. Sources remain
untouched; apply the complete reviewed diff with the existing patch tool. No
multi-file filesystem transaction or automatic capture qualification is claimed.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import os
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path


MAX_BYTES = 32 * 1024 * 1024
IDENTIFIER = re.compile(r"(?:r#)?[A-Za-z_][A-Za-z_0-9]*\Z")
DIGEST = re.compile(r"[0-9a-f]{64}\Z")
RAW_STRING = re.compile(r'(?:br|cr|r)(#*)"')
QUOTED_STRING = re.compile(r'(?:b|c)?"')
CHARACTER = re.compile(r"(?:b)?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
SOURCE_IDENTIFIER = re.compile(r"(?:r#)?[A-Za-z_][A-Za-z_0-9]*")
ORDINARY_ATTRIBUTES = frozenset({
    "allow", "warn", "deny", "forbid", "expect", "doc", "cfg", "deprecated",
    "must_use", "repr", "non_exhaustive", "derive", "norito", "schema",
    "getset", "display", "debug", "error", "from", "source", "backtrace",
})


class MappingError(ValueError):
    """An unverified or unsupported mapping cannot produce a patch."""


@dataclass(frozen=True)
class Token:
    value: str
    start: int
    end: int


@dataclass(frozen=True)
class Declaration:
    start: int
    end: int
    kind: str
    identifier: str
    generic: bool
    attributes: tuple[tuple[Token, ...], ...]
    unsupported_context: bool
    scopes: tuple[tuple[int, int], ...]


@dataclass(frozen=True)
class FilePatch:
    path: str
    current: bytes
    original: bytes
    result: bytes


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def strict_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise MappingError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def fields(value: object, required: set[str], optional: set[str] = frozenset()) -> dict:
    if not isinstance(value, dict) or not required <= value.keys() or value.keys() - required - optional:
        raise MappingError(f"expected fields {sorted(required)} with optional {sorted(optional)}")
    return value


def literal(value: object) -> str:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise MappingError("identity must be a nonempty literal without surrounding whitespace")
    if any(ord(char) < 32 or ord(char) == 127 or 0xD800 <= ord(char) <= 0xDFFF for char in value):
        raise MappingError("identity contains an invalid control or surrogate character")
    return value


def load_mapping(path: Path) -> list[dict]:
    raw = path.read_bytes()
    if len(raw) > MAX_BYTES:
        raise MappingError("mapping exceeds the size limit")
    document = fields(json.loads(raw, object_pairs_hook=strict_object), {"schema", "files"})
    if type(document["schema"]) is not int or document["schema"] != 1:
        raise MappingError("mapping schema must be 1")
    files = document["files"]
    if not isinstance(files, list) or not files:
        raise MappingError("files must be a nonempty list")
    paths: set[str] = set()
    identities: set[str] = set()
    for entry in files:
        fields(entry, {"path", "sha256", "declarations"})
        name = entry["path"]
        if (not isinstance(name, str) or re.fullmatch(r"[A-Za-z_0-9./-]+", name) is None
                or any(part in {"", ".", ".."} for part in name.split("/"))
                or not name.endswith(".rs")):
            raise MappingError("source path must be normalized, relative, and end in .rs")
        if name in paths:
            raise MappingError(f"duplicate source path: {name}")
        paths.add(name)
        if not isinstance(entry["sha256"], str) or not DIGEST.fullmatch(entry["sha256"]):
            raise MappingError(f"invalid source digest: {name}")
        declarations = entry["declarations"]
        if not isinstance(declarations, list) or not declarations:
            raise MappingError(f"declarations must be a nonempty list: {name}")
        for row in declarations:
            fields(row, {"start_byte", "end_byte", "anchor", "kind", "identifier", "nominal"}, {"frame"})
            start, end = row["start_byte"], row["end_byte"]
            if type(start) is not int or type(end) is not int or start < 0 or end <= start:
                raise MappingError(f"invalid declaration byte span: {name}")
            if not isinstance(row["anchor"], str) or not row["anchor"]:
                raise MappingError("anchor must contain the exact declaration source")
            if not isinstance(row["kind"], str) or row["kind"] not in {"struct", "enum"} or not isinstance(row["identifier"], str) or not IDENTIFIER.fullmatch(row["identifier"]):
                raise MappingError("only named ordinary structs and enums are supported")
            nominal = literal(row["nominal"])
            if nominal in identities:
                raise MappingError(f"duplicate nominal identity: {nominal}")
            identities.add(nominal)
            if "frame" in row:
                literal(row["frame"])
        entry["declarations"] = sorted(declarations, key=lambda row: row["start_byte"])
        previous_end = -1
        for row in entry["declarations"]:
            if row["start_byte"] < previous_end:
                raise MappingError(f"duplicate or overlapping declarations: {name}")
            previous_end = row["end_byte"]
    return sorted(files, key=lambda entry: entry["path"])


def read_source(root: Path, name: str) -> bytes:
    path = root
    for part in name.split("/"):
        path = path / part
        if stat.S_ISLNK(path.lstat().st_mode):
            raise MappingError(f"source symlinks are unsupported: {name}")
    descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_BYTES:
            raise MappingError(f"source must be a bounded regular file: {name}")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            data = stream.read(MAX_BYTES + 1)
        after = os.fstat(descriptor)
        current = path.lstat()
        identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns, item.st_ctime_ns)
        if len(data) > MAX_BYTES or identity(before) != identity(after) or identity(after) != identity(current):
            raise MappingError(f"source changed while reading: {name}")
        data.decode("utf-8")
        if b"\x00" in data:
            raise MappingError(f"NUL in Rust source: {name}")
        return data
    finally:
        os.close(descriptor)


def tokenize(source: bytes) -> list[Token]:
    """Lex the bounded Rust subset, excluding comments and treating literals atomically."""
    text = source.decode("utf-8")
    offsets = [0]
    for char in text:
        offsets.append(offsets[-1] + len(char.encode("utf-8")))
    tokens: list[Token] = []
    index = 0
    while index < len(text):
        start = index
        char = text[index]
        if char.isspace():
            index += 1
            continue
        if text.startswith("//", index):
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
            continue
        if text.startswith("/*", index):
            depth = 1
            index += 2
            while depth and index < len(text):
                if text.startswith("/*", index):
                    depth += 1
                    index += 2
                elif text.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            if depth:
                raise MappingError("unterminated Rust block comment")
            continue
        raw = RAW_STRING.match(text, index)
        quoted = QUOTED_STRING.match(text, index)
        if raw:
            closing = '"' + raw.group(1)
            end = text.find(closing, index + len(raw.group(0)))
            if end < 0:
                raise MappingError("unterminated Rust raw string")
            index = end + len(closing)
        elif quoted:
            index += len(quoted.group(0))
            while index < len(text):
                if text[index] == "\\":
                    index += 2
                elif text[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            else:
                raise MappingError("unterminated Rust string")
        else:
            character = CHARACTER.match(text, index)
            identifier = SOURCE_IDENTIFIER.match(text, index)
            if character:
                index += len(character.group(0))
            elif identifier:
                index += len(identifier.group(0))
            elif text.startswith("->", index):
                index += 2
            else:
                index += 1
        tokens.append(Token(text[start:index], offsets[start], offsets[index]))
    return tokens


def delimiters(tokens: list[Token]) -> dict[int, int]:
    stack = []
    pairs = {}
    for index, token in enumerate(tokens):
        if token.value in {"(", "[", "{"}:
            stack.append(index)
        elif token.value in {")", "]", "}"}:
            if not stack or tokens[stack[-1]].value != {")": "(", "]": "[", "}": "{"}[token.value]:
                raise MappingError("unbalanced Rust delimiters")
            opening = stack.pop()
            pairs[opening] = index
    if stack:
        raise MappingError("unbalanced Rust delimiters")
    return pairs


def ordinary_attribute(tokens: tuple[Token, ...]) -> bool:
    if not tokens:
        return False
    if tokens[0].value == "cfg_attr":
        # Attribute macros hidden behind a cfg are still unresolved mappings.
        if len(tokens) < 4 or tokens[1].value != "(" or tokens[-1].value != ")":
            return False
        inner = list(tokens[2:-1])
        pairs = delimiters(inner)
        pieces, begin, index = [], 0, 0
        while index < len(inner):
            if index in pairs:
                index = pairs[index] + 1
            elif inner[index].value == ",":
                pieces.append(tuple(inner[begin:index]))
                begin = index + 1
                index += 1
            else:
                index += 1
        if begin < len(inner):
            pieces.append(tuple(inner[begin:]))
        return len(pieces) >= 2 and all(ordinary_attribute(piece) for piece in pieces[1:])
    return tokens[0].value in ORDINARY_ATTRIBUTES and (len(tokens) == 1 or tokens[1].value in {"(", "="})


def declarations(source: bytes) -> tuple[dict[int, Declaration], list[Token]]:
    tokens = tokenize(source)
    pairs = delimiters(tokens)
    found = {}

    def walk(begin: int, end: int, unsupported: bool = False, ancestors: tuple[tuple[int, int], ...] = ()) -> None:
        scopes = (*ancestors, (begin, end))
        index = begin
        while index < end:
            if tokens[index].value == ";":
                index += 1
                continue
            attributes = []
            while index + 1 < end and tokens[index].value == "#":
                opening = index + 1
                inner_attribute = tokens[opening].value == "!"
                if inner_attribute:
                    opening += 1
                if opening not in pairs or tokens[opening].value != "[":
                    raise MappingError("unsupported Rust attribute syntax")
                closing = pairs[opening]
                attribute = tuple(tokens[opening + 1:closing])
                if inner_attribute:
                    unsupported = unsupported or not ordinary_attribute(attribute)
                else:
                    attributes.append(attribute)
                index = closing + 1
            if index >= end:
                break
            start = index
            if tokens[index].value == "pub":
                index += 1
                if index < end and tokens[index].value == "(":
                    index = pairs[index] + 1
            if index >= end:
                raise MappingError("incomplete Rust item")
            kind = tokens[index].value
            identifier = tokens[index + 1].value if index + 1 < end else ""
            scan = index + 1
            while scan < end:
                if tokens[scan].value in {";", "{"}:
                    break
                scan = pairs[scan] + 1 if scan in pairs else scan + 1
            if scan == end:
                break
            closing = pairs[scan] if tokens[scan].value == "{" else scan
            bad = unsupported or not all(ordinary_attribute(attr) for attr in attributes)
            if kind in {"struct", "enum"} and IDENTIFIER.fullmatch(identifier):
                generic = index + 2 < end and tokens[index + 2].value == "<"
                if generic:
                    level = 0
                    for parameter in tokens[index + 2:scan]:
                        if parameter.value == "<":
                            level += 1
                        elif parameter.value == ">":
                            level -= 1
                        if level == 0:
                            break
                    if level != 0:
                        bad = True
                found[tokens[start].start] = Declaration(
                    tokens[start].start, tokens[closing].end, kind, identifier,
                    generic,
                    tuple(attributes), bad, scopes,
                )
            elif kind == "mod" and tokens[scan].value == "{" and IDENTIFIER.fullmatch(identifier):
                walk(scan + 1, closing, bad, scopes)
            index = closing + 1

    walk(0, len(tokens))
    return found, tokens


def reject_unresolved_imports(item: Declaration, tokens: list[Token]) -> None:
    """Reject visible import forms whose identity/derive binding is unresolved."""
    pairs = delimiters(tokens)
    attribute_names = {token.value for attribute in item.attributes for token in attribute}
    for begin, end in item.scopes:
        index = begin
        while index < end:
            if tokens[index].value == "use":
                stop = index + 1
                while stop < end and tokens[stop].value != ";":
                    stop += 1
                imported = [token.value for token in tokens[index + 1:stop]]
                if "*" in imported:
                    raise MappingError("visible wildcard imports require semantic review")
                for offset, value in enumerate(imported[:-1]):
                    if value == "as" and imported[offset + 1] in attribute_names:
                        raise MappingError("imported derive aliases require semantic review")
                index = stop + 1
            else:
                index = pairs[index] + 1 if index in pairs else index + 1


def insertion(source: bytes, start: int, row: dict) -> bytes:
    line_start = source.rfind(b"\n", 0, start) + 1
    indent = source[line_start:start]
    if indent.strip(b" \t"):
        raise MappingError("declaration must start on its own indented line")
    line_end = source.find(b"\n", start)
    has_crlf = (line_end > 0 and source[line_end - 1:line_end] == b"\r") or (
        line_end < 0 and line_start >= 2 and source[line_start - 2:line_start] == b"\r\n"
    )
    newline = b"\r\n" if has_crlf else b"\n"
    name = json.dumps(row["nominal"], ensure_ascii=False)
    frame = ', frame = ' + json.dumps(row["frame"], ensure_ascii=False) if "frame" in row else ""
    return (b"#[derive(norito::NoritoSchema)]" + newline + indent
            + f"#[norito_schema(name = {name}{frame})]".encode("utf-8") + newline + indent)


def recover_original(current: bytes, entry: dict) -> bytes:
    if sha256(current) == entry["sha256"]:
        return current
    # The only accepted non-original state is the exact complete generated result.
    removed, shift = [], 0
    for row in entry["declarations"]:
        position = row["start_byte"] + shift
        prefix = insertion(current, position, row)
        if current[position:position + len(prefix)] != prefix:
            raise MappingError(f"stale source or partial application: {entry['path']}")
        removed.append((position, len(prefix)))
        shift += len(prefix)
    original = current
    for position, length in reversed(removed):
        original = original[:position] + original[position + length:]
    if sha256(original) != entry["sha256"]:
        raise MappingError(f"stale source digest: {entry['path']}")
    return original


def plan_file(root: Path, entry: dict) -> FilePatch:
    current = read_source(root, entry["path"])
    original = recover_original(current, entry)
    items, tokens = declarations(original)
    edits = []
    for row in entry["declarations"]:
        start, end = row["start_byte"], row["end_byte"]
        if original[start:end] != row["anchor"].encode("utf-8"):
            raise MappingError(f"declaration anchor mismatch: {entry['path']}:{start}")
        item = items.get(start)
        if item is None or (item.end, item.kind, item.identifier) != (end, row["kind"], row["identifier"]):
            raise MappingError(f"span is not one supported complete declaration: {entry['path']}:{start}")
        if any(token.value in {"NoritoSchema", "norito_schema"} for attribute in item.attributes for token in attribute):
            raise MappingError(f"existing identity declaration: {entry['path']}:{start}")
        if item.unsupported_context:
            raise MappingError(f"generated or unresolved declaration syntax/attributes: {entry['path']}:{start}")
        if item.generic and "frame" in row:
            raise MappingError("generic frame projections require separate review")
        reject_unresolved_imports(item, tokens)
        if any(token.value in {"!", "$"} for token in tokens if start <= token.start < end):
            raise MappingError(f"macro-containing declaration requires review: {entry['path']}:{start}")
        # Explicit manual identities and imported identity aliases need semantic review.
        for index, token in enumerate(tokens):
            if token.value != "NoritoSchema":
                continue
            tail = []
            for part in tokens[index + 1:]:
                if part.value in {";", "{"}:
                    break
                tail.append(part.value)
            if ("as" in tail[:2] or ("for" in tail and row["identifier"] in tail[tail.index("for") + 1:])):
                raise MappingError(f"manual or aliased identity requires review: {entry['path']}")
        edits.append((start, insertion(original, start, row)))
    result = original
    for position, prefix in reversed(edits):
        result = result[:position] + prefix + result[position:]
    if current != original and current != result:
        raise MappingError(f"source is not the exact generated result: {entry['path']}")
    return FilePatch(entry["path"], current, original, result)


def generate(root: Path, files: list[dict]) -> list[FilePatch]:
    patches = [plan_file(root, entry) for entry in files]
    if len({patch.current == patch.result for patch in patches}) > 1:
        raise MappingError("partial batch application requires review")
    if any(read_source(root, patch.path) != patch.current for patch in patches):
        raise MappingError("source changed during batch verification")
    return patches


def unified_patch(patches: list[FilePatch]) -> str:
    output = []
    for patch in patches:
        if patch.current == patch.result:
            continue
        for line in difflib.unified_diff(
            patch.current.decode("utf-8").splitlines(keepends=True),
            patch.result.decode("utf-8").splitlines(keepends=True),
            fromfile=f"a/{patch.path}", tofile=f"b/{patch.path}",
        ):
            output.append(line if line.endswith("\n") else line + "\n\\ No newline at end of file\n")
    return "".join(output)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("mapping", type=Path)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true", help="exit 1 when the verified patch is still pending")
    args = parser.parse_args(argv)
    try:
        patches = generate(args.root.resolve(strict=True), load_mapping(args.mapping))
        pending = sum(patch.current != patch.result for patch in patches)
        if not args.check:
            sys.stdout.write(unified_patch(patches))
        print(json.dumps({"pending_files": pending, "files": [
            {"path": patch.path, "original_sha256": sha256(patch.original), "result_sha256": sha256(patch.result)}
            for patch in patches
        ]}, sort_keys=True), file=sys.stderr)
        return int(args.check and pending > 0)
    except (MappingError, OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"identity mapping rejected: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
