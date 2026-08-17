#!/usr/bin/env python3
"""Guard the typed ``WorldReadOnly`` field-accessor schema in ``state.rs``.

The guard uses only the Python standard library.  It authenticates the direct
accessor preimage through a canonical expansion manifest, pins the fixed macro
emitters and surviving hand-written methods, and rejects source growth or an
escape hatch that could hide executable behavior in the schema.
"""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/state.rs"

PREIMAGE_INDEX_BLOB = "fd9b76316efb95be7c0411b5727db2b3fb0cebfb"
PREIMAGE_ACCESSOR_SURFACE_SHA256 = (
    "ebff8a03a5a264fef9cf85e413a8f0ec7e9cf7ca3318fad58d0e682443889525"
)
EXPANDED_PREIMAGE_SHA256 = (
    "d7c7bb0676d50eac9b5f9760fb0617370bc0621d3d40f075fb18ee7e0aa2be46"
)
SCHEMA_SOURCE_SHA256 = (
    "1e3cc306c9906e4470e5d26f7cf0d007f67ec0a6989554e62b31f85cff3e839a"
)
EMITTER_SOURCE_SHA256 = (
    "ce7152086f5bf3542e7ae9691781a0b919cfe34068dcced4fc26e62219f66f5c"
)
DEFAULT_METHODS_SHA256 = (
    "64927a5d22a02971a4f1e11b31c4df04f29db218fda1c6d1066ccfb88f4b5969"
)
SPECIAL_METHOD_SHA256 = (
    "6921cf62d0e4f822c6f1effddd7f89731f1f67e231f3cee3925b91a9f6d9ce50"
)
IMPL_WRAPPER_SHA256 = (
    "cf5a4a8c4e178c712694c622489aec289d65c54ea3d8c92511d50516bc8bd2f2"
)
IMPLEMENTERS_SHA256 = (
    "891de225447dd959ac0e998fce636e6cb116dfe22bce72faf8a27a8622ac10ba"
)

GROUPS = (
    ("configuration", 2),
    ("identity", 22),
    ("assets", 28),
    ("oracle_and_incentives", 15),
    ("escrow_and_outbound", 17),
    ("sccp_inbound", 2),
    ("runtime_and_proofs", 17),
    ("contract_uploads", 6),
    ("contract_state", 6),
    ("musubi_registry", 21),
    ("soracloud", 33),
    ("agreements_and_lanes", 15),
    ("sorafs_and_privacy", 25),
    ("governance", 16),
)
KIND_COUNTS = {
    "storage": 203,
    "ref": 15,
    "cell_ref": 4,
    "cell_copy": 2,
    "cell_inner": 1,
}
SPECIAL_METHOD = "privacy_bootle_lantern_issuer_policy_v1"
MAX_SCHEMA_LINES = 636
MAX_SCHEMA_AND_CALL_LINES = 664
MAX_IMPL_WRAPPER_LINES = 34
FORBIDDEN_SCHEMA_TOKENS = (
    "Box<dyn",
    "Fn(",
    "FnMut",
    "FnOnce",
    "fn(",
    "callback",
    "$action",
    "$body",
    "$setup",
    "Action",
    "Step",
)

GROUP_PATTERN = re.compile(
    r"(?ms)^    \((?P<name>[a-z_]+), \$mode:ident\) => \{\n"
    r"        world_ro_accessors!\(@items \$mode;\n"
    r"(?P<body>.*?)"
    r"^        \);\n"
    r"^    \};"
)
DECLARATION_CALL_PATTERN = re.compile(
    r"world_ro_accessors!\(([a-z_]+), declaration\);"
)
IMPLEMENTATION_CALL_PATTERN = re.compile(
    r"world_ro_accessors!\(([a-z_]+), implementation\);"
)


class GuardError(AssertionError):
    """The accessor schema no longer expands to its authenticated preimage."""


@dataclass(frozen=True)
class Accessor:
    """One fixed field accessor represented by the typed Rust schema."""

    group: str
    kind: str
    name: str
    docs: tuple[str, ...]
    parts: tuple[str, ...]


@dataclass(frozen=True)
class DirectMethod:
    """One hand-written trait or implementation method."""

    name: str
    kind: str
    chunk: str


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


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
        if re.match(r'(?:b?r)#+"', source[cursor:]):
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


def _accessors(schema: str) -> list[Accessor]:
    matches = list(GROUP_PATTERN.finditer(schema))
    observed_groups = tuple(match.group("name") for match in matches)
    expected_groups = tuple(name for name, _count in GROUPS)
    if observed_groups != expected_groups:
        raise GuardError(f"accessor groups changed: {observed_groups!r}")
    accessors: list[Accessor] = []
    for match, (group, expected_count) in zip(matches, GROUPS):
        entries = _parse_group(group, match.group("body"))
        if len(entries) != expected_count:
            raise GuardError(
                f"{group}: expected {expected_count} accessors, found {len(entries)}"
            )
        accessors.extend(entries)
    return accessors


def _direct_methods(
    source: str, opening: int, closing: int, indent: str
) -> list[DirectMethod]:
    pattern = re.compile(
        rf"^{re.escape(indent)}fn\s+([A-Za-z_][A-Za-z0-9_]*)", re.MULTILINE
    )
    methods: list[DirectMethod] = []
    for match in pattern.finditer(source, opening + 1, closing):
        start = match.start()
        cursor = match.end()
        parentheses = 0
        brackets = 0
        while cursor < closing:
            character = source[cursor]
            if character == "(":
                parentheses += 1
            elif character == ")":
                parentheses -= 1
            elif character == "[":
                brackets += 1
            elif character == "]":
                brackets -= 1
            elif character == ";" and parentheses == brackets == 0:
                kind = "declaration"
                end = cursor + 1
                break
            elif character == "{" and parentheses == brackets == 0:
                kind = "body"
                end = _matching_brace(source, cursor) + 1
                break
            cursor += 1
        else:
            raise GuardError(f"unterminated method {match.group(1)}")
        chunk_start = source.rfind("\n", 0, start) + 1
        while chunk_start:
            previous_start = source.rfind("\n", 0, chunk_start - 1) + 1
            previous = source[previous_start : chunk_start - 1]
            if previous.startswith(indent + "///") or previous.startswith(indent + "#["):
                chunk_start = previous_start
            else:
                break
        methods.append(DirectMethod(match.group(1), kind, source[chunk_start:end]))
    return methods


def _validate_hand_written_surface(source: str, accessors: list[Accessor]) -> None:
    _trait, trait_open, trait_close = _braced_item(source, "pub trait WorldReadOnly")
    _implementation, impl_open, impl_close = _braced_item(
        source, "impl WorldReadOnly for $ident"
    )
    trait_region = source[trait_open + 1 : trait_close]
    impl_region = source[impl_open + 1 : impl_close]
    expected_groups = [name for name, _count in GROUPS]
    if DECLARATION_CALL_PATTERN.findall(trait_region) != expected_groups:
        raise GuardError("trait accessor group call order changed")
    if IMPLEMENTATION_CALL_PATTERN.findall(impl_region) != expected_groups:
        raise GuardError("implementation accessor group call order changed")

    trait_methods = _direct_methods(source, trait_open, trait_close, "    ")
    impl_methods = _direct_methods(source, impl_open, impl_close, "            ")
    declarations = [method.name for method in trait_methods if method.kind == "declaration"]
    if declarations != [SPECIAL_METHOD]:
        raise GuardError(f"unexpected direct trait declarations: {declarations!r}")
    if [(method.name, method.kind) for method in impl_methods] != [(SPECIAL_METHOD, "body")]:
        raise GuardError("the impl wrapper must retain only the bespoke privacy method")

    defaults = "\n".join(
        method.chunk for method in trait_methods if method.kind == "body"
    )
    if _sha256(defaults) != DEFAULT_METHODS_SHA256:
        raise GuardError("hand-written default trait methods changed")
    trait_special = next(method.chunk for method in trait_methods if method.name == SPECIAL_METHOD)
    impl_special = impl_methods[0].chunk
    special = trait_special.strip("\n") + "\n" + impl_special.strip("\n")
    if _sha256(special) != SPECIAL_METHOD_SHA256:
        raise GuardError("the bespoke privacy accessor changed")

    accessor_names = {accessor.name for accessor in accessors}
    direct_names = {method.name for method in trait_methods + impl_methods}
    leaked = sorted(accessor_names & direct_names)
    if leaked:
        raise GuardError(f"schema accessors leaked back into direct Rust bodies: {leaked!r}")

    before_decl = trait_region.index(
        "world_ro_accessors!(sorafs_and_privacy, declaration);"
    )
    special_decl = trait_region.index(f"fn {SPECIAL_METHOD}")
    after_decl = trait_region.index("world_ro_accessors!(governance, declaration);")
    before_impl = impl_region.index(
        "world_ro_accessors!(sorafs_and_privacy, implementation);"
    )
    special_impl = impl_region.index(f"fn {SPECIAL_METHOD}")
    after_impl = impl_region.index("world_ro_accessors!(governance, implementation);")
    if not (before_decl < special_decl < after_decl and before_impl < special_impl < after_impl):
        raise GuardError("the bespoke method moved out of its historical position")


def validate_source(source: str) -> None:
    schema = _schema(source)
    schema_lines = len(schema.splitlines())
    call_count = len(DECLARATION_CALL_PATTERN.findall(source)) + len(
        IMPLEMENTATION_CALL_PATTERN.findall(source)
    )
    if schema_lines > MAX_SCHEMA_LINES:
        raise GuardError("WorldReadOnly schema exceeded its Rust-line budget")
    if schema_lines + call_count > MAX_SCHEMA_AND_CALL_LINES:
        raise GuardError("WorldReadOnly schema and call sites exceeded their line budget")
    for token in FORBIDDEN_SCHEMA_TOKENS:
        if token in schema:
            raise GuardError(f"accessor schema contains forbidden escape hatch {token!r}")
    if _sha256(schema) != SCHEMA_SOURCE_SHA256:
        raise GuardError("typed accessor schema source changed")

    emitter_start = schema.index("    // The schema has only fixed")
    emitter_end = schema.index("    (configuration, $mode:ident)")
    if _sha256(schema[emitter_start:emitter_end]) != EMITTER_SOURCE_SHA256:
        raise GuardError("fixed accessor emitters changed")
    accessors = _accessors(schema)
    if len(accessors) != 225:
        raise GuardError(f"expected 225 field accessors, found {len(accessors)}")
    names = [accessor.name for accessor in accessors]
    if len(set(names)) != len(names):
        raise GuardError("field accessor names must be unique")
    if Counter(accessor.kind for accessor in accessors) != Counter(KIND_COUNTS):
        raise GuardError("field accessor kind inventory changed")
    manifest = [
        (accessor.group, accessor.kind, accessor.name, accessor.docs, accessor.parts)
        for accessor in accessors
    ]
    encoded = json.dumps(manifest, separators=(",", ":"))
    if _sha256(encoded) != EXPANDED_PREIMAGE_SHA256:
        raise GuardError("schema no longer expands to the accessor preimage")

    impl_wrapper = _braced_item(source, "macro_rules! impl_world_ro")[0]
    if len(impl_wrapper.splitlines()) > MAX_IMPL_WRAPPER_LINES:
        raise GuardError("WorldReadOnly implementation wrapper exceeded its line budget")
    if _sha256(impl_wrapper) != IMPL_WRAPPER_SHA256:
        raise GuardError("WorldReadOnly implementation wrapper changed")
    implementers = """impl_world_ro! {
    WorldBlock<'_>, WorldTransaction<'_, '_>, WorldView<'_>
}"""
    if source.count(implementers) != 1 or _sha256(implementers) != IMPLEMENTERS_SHA256:
        raise GuardError("WorldReadOnly implementer order changed")
    _validate_hand_written_surface(source, accessors)


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class StateWorldReadOnlyAccessorSchemaSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def assert_rejected(self, source: str, message: str | None = None) -> None:
        if message is None:
            with self.assertRaises(GuardError):
                validate_source(source)
        else:
            with self.assertRaisesRegex(GuardError, message):
                validate_source(source)

    def test_current_source_matches_authenticated_accessor_preimage(self) -> None:
        validate_source(self.source)
        self.assertRegex(PREIMAGE_INDEX_BLOB, r"^[0-9a-f]{40}$")
        self.assertRegex(PREIMAGE_ACCESSOR_SURFACE_SHA256, r"^[0-9a-f]{64}$")

    def test_rustdoc_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "/// Global parameters registry.",
            "/// Mutable parameters registry.",
        )
        self.assert_rejected(mutated)

    def test_signature_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "storage domains: DomainId => Domain;",
            "storage domains: DomainId => AccountValue;",
        )
        self.assert_rejected(mutated)

    def test_group_call_order_mutation_is_rejected(self) -> None:
        original = """    world_ro_accessors!(assets, declaration);
    world_ro_accessors!(oracle_and_incentives, declaration);"""
        replacement = """    world_ro_accessors!(oracle_and_incentives, declaration);
    world_ro_accessors!(assets, declaration);"""
        self.assert_rejected(_replace_once(self.source, original, replacement))

    def test_emitter_body_mutation_is_rejected(self) -> None:
        original = """        fn $name(&self) -> &impl StorageReadOnly<$key, $value> {
            &self.$name
        }"""
        replacement = original.replace("&self.$name", "self.$name.get()")
        self.assert_rejected(_replace_once(self.source, original, replacement))

    def test_special_method_mutation_is_rejected(self) -> None:
        original = "crate::privacy_state::load_privacy_bootle_lantern_issuer_policy_v1("
        replacement = "crate::privacy_state::load_privacy_bootle_lantern_issuer_policy_v0("
        self.assert_rejected(_replace_once(self.source, original, replacement))

    def test_implementer_mutation_is_rejected(self) -> None:
        original = "WorldBlock<'_>, WorldTransaction<'_, '_>, WorldView<'_>"
        replacement = "WorldView<'_>, WorldTransaction<'_, '_>, WorldBlock<'_>"
        self.assert_rejected(_replace_once(self.source, original, replacement))

    def test_callback_escape_hatch_is_rejected(self) -> None:
        marker = (
            "    // The schema has only fixed, typed field-access forms; "
            "executable methods stay in the trait."
        )
        mutated = _replace_once(self.source, marker, marker + " callback")
        self.assert_rejected(mutated, "forbidden escape hatch")

    def test_schema_line_growth_is_rejected(self) -> None:
        marker = "macro_rules! world_ro_accessors {\n"
        mutated = _replace_once(self.source, marker, marker + "\n")
        self.assert_rejected(mutated, "Rust-line budget")


if __name__ == "__main__":
    unittest.main()
