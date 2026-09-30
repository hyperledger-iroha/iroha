#!/usr/bin/env python3
"""Check the current typed WorldReadOnly schema and read-only emitter ownership.

This guard checks enduring schema structure while permitting reviewed fields,
Rustdoc and hand-written methods to evolve with the first-release model.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_core/src/state.rs"
RAW_STRING_PREFIX = re.compile(r'(?:b?r)#+"')
GROUP_PATTERN = re.compile(
    r"(?ms)^    \((?P<name>[a-z_]+), \$mode:ident\) => \{\n"
    r"        world_ro_accessors!\(@items \$mode;\n"
    r"(?P<body>.*?)"
    r"^        \);\n"
    r"^    \};"
)
CALL_PATTERN = re.compile(r"world_ro_accessors!\(([a-z_]+), (declaration|implementation)\);")


class GuardError(AssertionError):
    """The schema violates its typed read-only ownership contract."""


@dataclass(frozen=True)
class Accessor:
    """One documented typed field-access row."""

    group: str
    kind: str
    name: str
    docs: tuple[str, ...]
    parts: tuple[str, ...]


def _skip_quoted(source: str, index: int) -> int:
    raw = re.match(r'(?:b?r)(#*)"', source[index:])
    if raw:
        terminator = '"' + raw.group(1)
        end = source.find(terminator, index + raw.end())
        if end < 0:
            raise GuardError("unterminated Rust raw string")
        return end + len(terminator)
    quote_index = index + (1 if source.startswith('b"', index) else 0)
    quote = source[quote_index]
    cursor = quote_index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    raise GuardError("unterminated Rust quoted literal")


def _matching_brace(source: str, start: int) -> int:
    if source[start] != "{":
        raise GuardError("brace matcher did not start on an opening brace")
    depth = 0
    cursor = start
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            comment_depth = 1
            cursor += 2
            while cursor < len(source) and comment_depth:
                if source.startswith("/*", cursor):
                    comment_depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    comment_depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if comment_depth:
                raise GuardError("unterminated Rust block comment")
            continue
        if source[cursor] == '"' or source.startswith(('b"', 'r"', 'br"'), cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        if RAW_STRING_PREFIX.match(source, cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            closing = cursor + 2 if source[cursor + 1] != "\\" else cursor + 3
            if closing < len(source) and source[closing] == "'":
                cursor = closing + 1
                continue
        if source[cursor] == "{":
            depth += 1
        elif source[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust brace region")


def _braced_item(source: str, marker: str) -> tuple[str, int, int]:
    if source.count(marker) != 1:
        raise GuardError(f"expected one source marker: {marker!r}")
    start = source.index(marker)
    opening = source.index("{", start + len(marker))
    closing = _matching_brace(source, opening)
    return source[start : closing + 1], opening, closing


def _schema(source: str) -> str:
    return _braced_item(source, "macro_rules! world_ro_accessors")[0]


def _parse_group(group: str, body: str) -> list[Accessor]:
    lines = body.splitlines()
    entries: list[Accessor] = []
    cursor = 0
    doc_prefix = "            ///"
    while cursor < len(lines):
        docs: list[str] = []
        while cursor < len(lines) and lines[cursor].startswith(doc_prefix):
            docs.append(lines[cursor][len(doc_prefix) :])
            cursor += 1
        if not docs:
            raise GuardError(f"{group}: every accessor must retain its Rustdoc")
        fragments: list[str] = []
        while cursor < len(lines):
            fragment = lines[cursor].strip()
            fragments.append(fragment)
            cursor += 1
            if fragment.endswith(";"):
                break
        if not fragments[-1].endswith(";"):
            raise GuardError(f"{group}: unterminated accessor row")
        row = " ".join(fragments)
        match = re.fullmatch(
            r"(storage|ref|cell_ref|cell_copy|cell_inner) "
            r"([A-Za-z_][A-Za-z0-9_]*): (.+);",
            row,
        )
        if match is None:
            raise GuardError(f"{group}: invalid typed accessor row {row!r}")
        kind, name, payload = match.groups()
        if kind == "storage":
            parts = tuple(part.strip() for part in payload.split(" => "))
            if len(parts) != 2:
                raise GuardError(f"{group}/{name}: storage row needs one key/value boundary")
        else:
            parts = (payload.strip(),)
        if any(not part or part.endswith(",") for part in parts):
            raise GuardError(f"{group}/{name}: non-canonical type fragment")
        entries.append(Accessor(group, kind, name, tuple(docs), parts))
    return entries


def validate_source(source: str) -> None:
    """Require each schema group to be documented, typed and wired exactly once."""

    schema = _schema(source)
    matches = list(GROUP_PATTERN.finditer(schema))
    groups = [match.group("name") for match in matches]
    if not groups or len(set(groups)) != len(groups):
        raise GuardError("accessor groups must be nonempty and unique")
    accessors = [
        accessor
        for match in matches
        for accessor in _parse_group(match.group("name"), match.group("body"))
    ]
    names = [accessor.name for accessor in accessors]
    if len(names) != len(set(names)):
        raise GuardError("field accessor names must be unique")

    emitter = schema[:matches[0].start()]
    if any(token in emitter for token in ("Fn(", "FnMut", "FnOnce", "dyn Fn", "$body", "$action")):
        raise GuardError("read-only emitters must not accept executable callbacks")
    # The fixed code-generation forms retain the same return types and physical
    # field owners. Field additions cannot introduce an arbitrary executable row.
    getters = {
        "storage": ("&impl StorageReadOnly<$key, $value>", "&self.$name"),
        "ref": ("&$value", "&self.$name"),
        "cell_ref": ("&$value", "self.$name.get()"),
        "cell_copy": ("$value", "*self.$name.get()"),
        "cell_inner": ("$value", "self.$name.get().get()"),
    }
    normalized = re.sub(r"\s+", "", emitter)
    if len(re.findall(r"\bfn\b", emitter)) != 2 * len(getters):
        raise GuardError("read-only emitters must contain only their fixed field methods")
    for kind, (return_type, expression) in getters.items():
        marker = f"(@itemsimplementation;$(#[$meta:meta])*{kind}$name:ident:"
        start = normalized.find(marker)
        if start < 0:
            raise GuardError(f"missing read-only emitter: {kind}")
        end = normalized.find("};", start)
        arm = normalized[start:end + 2]
        expected = re.sub(r"\s+", "", f"fn $name(&self) -> {return_type} {{ {expression} }}")
        if expected not in arm:
            raise GuardError(f"read-only emitter behavior changed: {kind}")
        if arm.count("fn$name(") != 1 or "$($rest:tt)*" not in arm:
            raise GuardError(f"read-only emitter structure changed: {kind}")

    trait = _braced_item(source, "pub trait WorldReadOnly")[0]
    implementation = _braced_item(source, "impl WorldReadOnly for $ident")[0]
    declared = [name for name, mode in CALL_PATTERN.findall(trait) if mode == "declaration"]
    implemented = [name for name, mode in CALL_PATTERN.findall(implementation) if mode == "implementation"]
    if declared != groups or implemented != groups:
        raise GuardError("trait and implementation must wire every schema group once in order")
    required_views = ("WorldBlock<'_>", "WorldTransaction<'_, '_>", "Box<WorldTransaction<'_, '_>>", "WorldView<'_>")
    calls = re.findall(r"impl_world_ro!\s*\{([^}]+)\}", source)
    if len(calls) != 1 or any(view not in calls[0] for view in required_views):
        raise GuardError("all physical world views must implement read-only accessors")


class StateWorldReadOnlyAccessorSchemaSourceTests(unittest.TestCase):
    """Current-source and adverse controls for typed read-only field access."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")

    def test_current_schema_contract(self) -> None:
        validate_source(self.source)

    def test_documentation_may_evolve_without_changing_access(self) -> None:
        validate_source(self.source.replace("/// Global parameters registry.", "/// Current global parameters.", 1))

    def test_every_accessor_requires_documentation(self) -> None:
        with self.assertRaisesRegex(GuardError, "Rustdoc"):
            validate_source(self.source.replace("            /// Global parameters registry.\n", "", 1))

    def test_duplicate_accessor_is_rejected(self) -> None:
        with self.assertRaisesRegex(GuardError, "unique"):
            validate_source(self.source.replace("cell_ref peers: Peers;", "cell_ref parameters: Peers;", 1))

    def test_malformed_type_boundary_is_rejected(self) -> None:
        with self.assertRaisesRegex(GuardError, "key/value boundary"):
            validate_source(self.source.replace("storage domains: DomainId => Domain;", "storage domains: DomainId;", 1))

    def test_executable_schema_row_is_rejected(self) -> None:
        with self.assertRaisesRegex(GuardError, "typed accessor row"):
            validate_source(self.source.replace("cell_ref parameters: Parameters;", "callback parameters: FnMut();", 1))

    def test_emitter_cannot_replace_physical_field_owner(self) -> None:
        with self.assertRaisesRegex(GuardError, "emitter behavior"):
            validate_source(self.source.replace("fn $name(&self) -> &impl StorageReadOnly<$key, $value> {\n            &self.$name", "fn $name(&self) -> &impl StorageReadOnly<$key, $value> {\n            self.$name.get()", 1))

    def test_group_wiring_cannot_be_omitted(self) -> None:
        with self.assertRaisesRegex(GuardError, "wire every schema group"):
            validate_source(self.source.replace("world_ro_accessors!(assets, declaration);", "", 1))

    def test_emitter_cannot_add_arbitrary_method(self) -> None:
        with self.assertRaisesRegex(GuardError, "only their fixed field methods"):
            validate_source(self.source.replace("            &self.$name\n        }", "            &self.$name\n        }\n        fn mutate(&self) {}", 1))

    def test_required_world_view_cannot_be_omitted(self) -> None:
        with self.assertRaisesRegex(GuardError, "all physical world views"):
            validate_source(self.source.replace("Box<WorldTransaction<'_, '_>>, WorldView<'_>", "Box<WorldTransaction<'_, '_>>", 1))


if __name__ == "__main__":
    unittest.main()
