#!/usr/bin/env python3
"""Regenerate canonical Kotodama V1 syntax, editor, and SDK policy blocks.

Prerequisites: Python 3.11+ and a repository checkout containing the canonical
``fixtures/kotodama_v1_policy.json`` descriptor and lexical ``v1.lex`` table.
The generator invokes neither Cargo nor SDK build tools, never reads the clock,
and never modifies ``Cargo.lock``. Generated regions are marker-delimited and
the command fails closed when a marker is missing or duplicated.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import difflib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
from typing import Any, Mapping, Sequence


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = Path("fixtures/kotodama_v1_policy.json")
POLICY_FORMAT = "iroha.kotodama.v1.policy"
POLICY_SCHEMA = 1

SEMANTIC_PATH = Path("crates/kotodama_lang/src/semantic.rs")
GRAMMAR_DOC_PATH = Path("docs/source/kotodama_grammar.md")
TEXTMATE_PATH = Path(
    "tools/kotodama_linguist/grammar-repo/syntaxes/kotodama.tmLanguage.json"
)
JAVASCRIPT_PATHS = (
    Path("javascript/iroha_js/src/numericV1.js"),
    Path("javascript/iroha_js/dist/numericV1.js"),
)
TYPESCRIPT_PATH = Path("javascript/iroha_js/index.d.ts")
PYTHON_PATH = Path("python/iroha_python/src/iroha_python/numeric_v1.py")
KOTLIN_PATH = Path(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/numeric/NumericV1.kt"
)
JAVA_PATH = Path(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/numeric/NumericV1.java"
)
SWIFT_PATH = Path("IrohaSwift/Sources/IrohaSwift/NumericV1.swift")
CSHARP_PATH = Path("csharp/src/Hyperledger.Iroha.Sdk/Numeric/NumericV1.cs")
JAVASCRIPT_IDENTIFIER_PATHS = (
    Path("javascript/iroha_js/src/kotodamaIdentifiers.js"),
    Path("javascript/iroha_js/dist/kotodamaIdentifiers.js"),
)
PYTHON_MANIFEST_PATH = Path("python/iroha_python/src/iroha_python/client.py")
KOTLIN_MANIFEST_PATH = Path(
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/ContractManifestModels.kt"
)
JAVA_MANIFEST_PATH = Path(
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ContractManifestJsonParser.java"
)
SWIFT_MANIFEST_PATH = Path("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift")
CSHARP_MANIFEST_PATH = Path(
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiContractManifestJson.cs"
)

GENERATED_TARGETS = (
    SEMANTIC_PATH,
    GRAMMAR_DOC_PATH,
    TEXTMATE_PATH,
    *JAVASCRIPT_PATHS,
    TYPESCRIPT_PATH,
    PYTHON_PATH,
    KOTLIN_PATH,
    JAVA_PATH,
    SWIFT_PATH,
    CSHARP_PATH,
    *JAVASCRIPT_IDENTIFIER_PATHS,
    PYTHON_MANIFEST_PATH,
    KOTLIN_MANIFEST_PATH,
    JAVA_MANIFEST_PATH,
    SWIFT_MANIFEST_PATH,
    CSHARP_MANIFEST_PATH,
)

MARKER_NAME = "kotodama-v1-numeric-policy"
VALIDATOR_MARKER_NAME = "kotodama-v1-validator-policy"
SEMANTIC_MARKERS = (
    "// BEGIN GENERATED: kotodama-v1-semantic-policy",
    "// END GENERATED: kotodama-v1-semantic-policy",
)
SOURCE_DOC_MARKERS = (
    "<!-- BEGIN GENERATED: kotodama-v1-source-policy -->",
    "<!-- END GENERATED: kotodama-v1-source-policy -->",
)
KEYWORD_DOC_MARKERS = (
    "<!-- BEGIN GENERATED: kotodama-v1-keywords -->",
    "<!-- END GENERATED: kotodama-v1-keywords -->",
)
OPERATOR_DOC_MARKERS = (
    "<!-- BEGIN GENERATED: kotodama-v1-operators -->",
    "<!-- END GENERATED: kotodama-v1-operators -->",
)

EXPECTED_NUMERIC_ABI = {
    "int": ("Int", 0x0011),
    "decimal": ("Decimal", 0x0012),
    "quantity": ("Quantity", 0x0010),
}
HEX_128 = re.compile(r"^[0-9a-f]{32}$")
POINTER_ID = re.compile(r"^0x[0-9a-f]{4}$")
IDENTIFIER = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


class GenerationError(RuntimeError):
    """Raised when canonical input or a generated region is malformed."""


@dataclass(frozen=True)
class NumericSchema:
    """One canonical Numeric V1 source/Rust/ABI mapping."""

    source_type: str
    rust_type: str
    schema_name: str
    schema_hash: str
    pointer_id: int
    scaled: bool


@dataclass(frozen=True)
class Policy:
    """Strictly validated, timestamp-free Kotodama V1 policy."""

    lexical_grammar: Path
    active_source_types: tuple[str, ...]
    retired_numeric_type_spellings: tuple[str, ...]
    ordinary_value_identifiers: tuple[str, ...]
    declaration_reserved_extras: tuple[str, ...]
    numeric_suffix_fixits: tuple[str, ...]
    sum_paths: tuple[str, ...]
    rounding_paths: tuple[str, ...]
    list_member_names: tuple[str, ...]
    editor_member_calls: tuple[str, ...]
    numeric_schemas: tuple[NumericSchema, ...]
    first_known_pointer: int
    last_assigned_pointer: int
    unassigned_pointers: tuple[int, ...]


@dataclass(frozen=True)
class LexicalGrammar:
    """Canonical lexical records in source order."""

    keywords: tuple[tuple[str, str], ...]
    operators: tuple[tuple[str, str], ...]


def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise GenerationError(f"failed to read {path}: {error}") from error


def _strict_keys(raw: Mapping[str, Any], expected: set[str], context: str) -> None:
    missing = sorted(expected - set(raw))
    unknown = sorted(set(raw) - expected)
    if missing:
        raise GenerationError(f"{context} is missing keys: {', '.join(missing)}")
    if unknown:
        raise GenerationError(f"{context} has unknown keys: {', '.join(unknown)}")


def _string_list(
    raw: object, context: str, *, nonempty: bool = True
) -> tuple[str, ...]:
    if not isinstance(raw, list) or (nonempty and not raw):
        qualifier = "non-empty " if nonempty else ""
        raise GenerationError(f"{context} must be a {qualifier}array")
    values: list[str] = []
    for index, value in enumerate(raw):
        if not isinstance(value, str) or not value:
            raise GenerationError(f"{context}[{index}] must be a non-empty string")
        values.append(value)
    if len(set(values)) != len(values):
        raise GenerationError(f"{context} contains duplicate values")
    return tuple(values)


def _relative_path(raw: object, context: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise GenerationError(f"{context} must be a non-empty path string")
    path = Path(raw)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise GenerationError(
            f"{context} must be a normalized repository-relative path"
        )
    return path


def _pointer_id(raw: object, context: str) -> int:
    if not isinstance(raw, str) or POINTER_ID.fullmatch(raw) is None:
        raise GenerationError(
            f"{context} must use lowercase four-digit hex, got {raw!r}"
        )
    return int(raw, 16)


def load_policy(root: Path, relative_path: Path = DEFAULT_POLICY) -> Policy:
    """Load and validate the canonical policy descriptor."""

    relative_path = _relative_path(str(relative_path), "policy path")
    path = root / relative_path
    try:
        raw = json.loads(_read_text(path))
    except json.JSONDecodeError as error:
        raise GenerationError(f"invalid policy JSON {path}: {error}") from error
    if not isinstance(raw, dict):
        raise GenerationError("policy root must be an object")
    _strict_keys(
        raw,
        {
            "format",
            "schema",
            "lexical_grammar",
            "active_source_types",
            "retired_numeric_type_spellings",
            "ordinary_value_identifiers",
            "declaration_reserved_extras",
            "numeric_suffix_fixits",
            "sum_paths",
            "rounding_paths",
            "list_member_names",
            "editor_member_calls",
            "numeric_schemas",
            "pointer_policy",
        },
        "policy",
    )
    if raw["format"] != POLICY_FORMAT:
        raise GenerationError(f"policy format must be {POLICY_FORMAT!r}")
    if type(raw["schema"]) is not int or raw["schema"] != POLICY_SCHEMA:
        raise GenerationError(f"policy schema must be {POLICY_SCHEMA}")

    active_types = _string_list(raw["active_source_types"], "active_source_types")
    retired_types = _string_list(
        raw["retired_numeric_type_spellings"], "retired_numeric_type_spellings"
    )
    ordinary_identifiers = _string_list(
        raw["ordinary_value_identifiers"], "ordinary_value_identifiers"
    )
    for context, values in [
        ("active_source_types", active_types),
        ("retired_numeric_type_spellings", retired_types),
        ("ordinary_value_identifiers", ordinary_identifiers),
        (
            "declaration_reserved_extras",
            _string_list(
                raw["declaration_reserved_extras"], "declaration_reserved_extras"
            ),
        ),
    ]:
        for value in values:
            if IDENTIFIER.fullmatch(value) is None:
                raise GenerationError(
                    f"{context} contains invalid identifier {value!r}"
                )

    suffixes = raw["numeric_suffix_fixits"]
    if not isinstance(suffixes, dict):
        raise GenerationError("numeric_suffix_fixits must be an object")
    if suffixes != {"amt": "strip", "qty": "strip"}:
        raise GenerationError(
            "numeric_suffix_fixits must contain only amt/qty safe strip fix-its"
        )

    schemas_raw = raw["numeric_schemas"]
    if not isinstance(schemas_raw, list) or not schemas_raw:
        raise GenerationError("numeric_schemas must be a non-empty array")
    schemas: list[NumericSchema] = []
    for index, item in enumerate(schemas_raw):
        context = f"numeric_schemas[{index}]"
        if not isinstance(item, dict):
            raise GenerationError(f"{context} must be an object")
        _strict_keys(
            item,
            {
                "source_type",
                "rust_type",
                "schema_name",
                "schema_hash",
                "pointer_id",
                "scaled",
            },
            context,
        )
        for field in ("source_type", "rust_type", "schema_name", "schema_hash"):
            if not isinstance(item[field], str) or not item[field]:
                raise GenerationError(f"{context}.{field} must be a non-empty string")
        if HEX_128.fullmatch(item["schema_hash"]) is None:
            raise GenerationError(
                f"{context}.schema_hash must be 16 lowercase hex bytes"
            )
        if type(item["scaled"]) is not bool:
            raise GenerationError(f"{context}.scaled must be boolean")
        schemas.append(
            NumericSchema(
                source_type=item["source_type"],
                rust_type=item["rust_type"],
                schema_name=item["schema_name"],
                schema_hash=item["schema_hash"],
                pointer_id=_pointer_id(item["pointer_id"], f"{context}.pointer_id"),
                scaled=item["scaled"],
            )
        )

    pointer_raw = raw["pointer_policy"]
    if not isinstance(pointer_raw, dict):
        raise GenerationError("pointer_policy must be an object")
    _strict_keys(
        pointer_raw, {"first_known", "last_assigned", "unassigned"}, "pointer_policy"
    )
    unassigned_raw = pointer_raw["unassigned"]
    if not isinstance(unassigned_raw, list):
        raise GenerationError("pointer_policy.unassigned must be an array")
    unassigned = tuple(
        _pointer_id(value, f"pointer_policy.unassigned[{index}]")
        for index, value in enumerate(unassigned_raw)
    )
    if len(set(unassigned)) != len(unassigned):
        raise GenerationError("pointer_policy.unassigned contains duplicates")

    policy = Policy(
        lexical_grammar=_relative_path(raw["lexical_grammar"], "lexical_grammar"),
        active_source_types=active_types,
        retired_numeric_type_spellings=retired_types,
        ordinary_value_identifiers=ordinary_identifiers,
        declaration_reserved_extras=_string_list(
            raw["declaration_reserved_extras"], "declaration_reserved_extras"
        ),
        numeric_suffix_fixits=tuple(suffixes),
        sum_paths=_string_list(raw["sum_paths"], "sum_paths"),
        rounding_paths=_string_list(raw["rounding_paths"], "rounding_paths"),
        list_member_names=_string_list(raw["list_member_names"], "list_member_names"),
        editor_member_calls=_string_list(
            raw["editor_member_calls"], "editor_member_calls"
        ),
        numeric_schemas=tuple(schemas),
        first_known_pointer=_pointer_id(
            pointer_raw["first_known"], "pointer_policy.first_known"
        ),
        last_assigned_pointer=_pointer_id(
            pointer_raw["last_assigned"], "pointer_policy.last_assigned"
        ),
        unassigned_pointers=unassigned,
    )
    _validate_final_v1_policy(policy)
    return policy


def _validate_final_v1_policy(policy: Policy) -> None:
    schema_sources = tuple(schema.source_type for schema in policy.numeric_schemas)
    if schema_sources != ("int", "decimal", "quantity"):
        raise GenerationError("numeric_schemas must be ordered int, decimal, quantity")
    if len({schema.pointer_id for schema in policy.numeric_schemas}) != len(
        policy.numeric_schemas
    ):
        raise GenerationError("numeric_schemas contains duplicate pointer IDs")
    for schema in policy.numeric_schemas:
        expected = EXPECTED_NUMERIC_ABI.get(schema.source_type)
        if expected != (schema.rust_type, schema.pointer_id):
            raise GenerationError(
                f"{schema.source_type} must map to Rust {expected[0]} at "
                f"0x{expected[1]:04x}"
            )
        expected_scaled = schema.source_type != "int"
        if schema.scaled != expected_scaled:
            raise GenerationError(
                f"{schema.source_type}.scaled must be {str(expected_scaled).lower()}"
            )
        if schema.source_type not in policy.active_source_types:
            raise GenerationError(
                f"active_source_types omits numeric type {schema.source_type!r}"
            )
    if (
        "quantity" not in policy.active_source_types
        or "Quantity" in policy.active_source_types
    ):
        raise GenerationError(
            "the public nominal source type must be lowercase quantity"
        )
    if "Amount" not in policy.retired_numeric_type_spellings:
        raise GenerationError(
            "Amount must remain rejected as retired source type syntax"
        )
    if "amount" not in policy.ordinary_value_identifiers:
        raise GenerationError("amount must remain an ordinary value identifier")
    if policy.first_known_pointer != 0x0001 or policy.last_assigned_pointer != 0x0012:
        raise GenerationError("V1 known pointer range must be 0x0001 through 0x0012")
    if policy.unassigned_pointers != (0x0013,):
        raise GenerationError(
            "0x0013 must be the sole explicitly unassigned V1 boundary"
        )
    if any(
        pointer <= policy.last_assigned_pointer
        for pointer in policy.unassigned_pointers
    ):
        raise GenerationError("unassigned pointer IDs overlap the assigned range")
    if any(name not in policy.editor_member_calls for name in policy.list_member_names):
        raise GenerationError("editor_member_calls omits a bounded List member")


def load_lexical_grammar(root: Path, policy: Policy) -> LexicalGrammar:
    """Parse the same tab-separated grammar consumed by build.rs."""

    path = root / policy.lexical_grammar
    keywords: list[tuple[str, str]] = []
    operators: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number, raw_line in enumerate(_read_text(path).splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("//"):
            continue
        columns = raw_line.split("\t")
        if len(columns) != 3 or columns[0] not in {"keyword", "operator"}:
            raise GenerationError(
                f"{path}:{line_number}: expected a tab-separated lexical record"
            )
        kind, spelling, variant = columns
        if not spelling:
            raise GenerationError(f"{path}:{line_number}: empty lexical spelling")
        if (
            not variant
            or not variant[0].isupper()
            or not all(
                character.isascii() and character.isalnum() for character in variant
            )
        ):
            raise GenerationError(
                f"{path}:{line_number}: invalid token variant {variant!r}"
            )
        if spelling in seen:
            raise GenerationError(
                f"{path}:{line_number}: duplicate lexical spelling {spelling!r}"
            )
        seen.add(spelling)
        (keywords if kind == "keyword" else operators).append((spelling, variant))

    branded = {
        "Seiyaku": ("seiyaku", "誓約"),
        "Kotoage": ("kotoage", "言挙げ"),
        "Hajimari": ("hajimari", "始まり"),
        "Kaizen": ("kaizen", "改善"),
    }
    for variant, expected in branded.items():
        actual = tuple(spelling for spelling, token in keywords if token == variant)
        if actual != expected:
            raise GenerationError(
                f"{variant} must have exactly its romanized and Japanese spellings"
            )
    for retired in ("contract", "entry", "init", "upgrade"):
        if retired in seen:
            raise GenerationError(f"retired English keyword {retired!r} is forbidden")
    for ordinary in policy.ordinary_value_identifiers:
        if ordinary in {spelling for spelling, _ in keywords}:
            raise GenerationError(
                f"ordinary value identifier {ordinary!r} became a keyword"
            )
    return LexicalGrammar(tuple(keywords), tuple(operators))


def _replace_generated(
    text: str, start: str, end: str, body: str, *, path: Path
) -> str:
    if text.count(start) != 1:
        raise GenerationError(f"{path}: expected exactly one marker {start!r}")
    if text.count(end) != 1:
        raise GenerationError(f"{path}: expected exactly one marker {end!r}")
    prefix, remainder = text.split(start, 1)
    if end not in remainder:
        raise GenerationError(f"{path}: end marker precedes start marker")
    _, suffix = remainder.split(end, 1)
    if not suffix.startswith(("\n", "\r\n", ",")):
        raise GenerationError(f"{path}: malformed generated region after {end!r}")
    return f"{prefix}{start}\n{body.rstrip()}\n{end}{suffix}"


def _rust_array(name: str, values: Sequence[str], doc: str) -> str:
    if name == "V1_SUM_PATHS":
        quoted = ", ".join(json.dumps(value, ensure_ascii=False) for value in values)
        return f"/// {doc}\npub const {name}: &[&str] = &[{quoted}];"
    rows = "\n".join(
        f"    {json.dumps(value, ensure_ascii=False)}," for value in values
    )
    return f"/// {doc}\npub const {name}: &[&str] = &[\n{rows}\n];"


def render_semantic_policy(policy: Policy) -> str:
    sections = [
        _rust_array(
            "V1_SOURCE_TYPE_NAMES",
            policy.active_source_types,
            "Canonical source-level type spellings offered by language tooling.",
        ),
        (
            "/// Retired pre-release numeric type spellings that remain reserved in V1.\n"
            "///\n"
            "/// Keeping these names unavailable to source-unit identities and declared\n"
            "/// types prevents authenticated metadata from reinterpreting a known retired\n"
            "/// type spelling. They remain ordinary names in value and function namespaces,\n"
            "/// including entrypoints.\n"
            + _rust_array(
                "V1_RETIRED_NUMERIC_TYPE_NAMES",
                policy.retired_numeric_type_spellings,
                "",
            ).split("\n", 1)[1]
        ),
        _rust_array(
            "V1_SUM_PATHS",
            policy.sum_paths,
            "Canonical active-only sum constructor and pattern paths.",
        ),
        _rust_array(
            "V1_ROUNDING_PATHS",
            policy.rounding_paths,
            "Canonical explicit exact-decimal rounding modes.",
        ),
        _rust_array(
            "V1_LIST_MEMBER_NAMES",
            policy.list_member_names,
            "Canonical bounded-list member API.",
        ),
    ]
    return "\n".join(sections)


def _markdown_code(value: str) -> str:
    escaped = value.replace("|", "\\|")
    return f"`{escaped}`"


def render_keyword_docs(grammar: LexicalGrammar) -> str:
    lines = ["| Spelling | Token |", "| --- | --- |"]
    lines.extend(
        f"| {_markdown_code(spelling)} | `{variant}` |"
        for spelling, variant in grammar.keywords
    )
    return "\n".join(lines)


def render_operator_docs(grammar: LexicalGrammar) -> str:
    lines = ["| Spelling |", "| --- |"]
    lines.extend(f"| {_markdown_code(spelling)} |" for spelling, _ in grammar.operators)
    return "\n".join(lines)


def render_source_policy_docs(policy: Policy) -> str:
    active = ", ".join(_markdown_code(value) for value in policy.active_source_types)
    retired = ", ".join(
        _markdown_code(value) for value in policy.retired_numeric_type_spellings
    )
    ordinary = ", ".join(
        _markdown_code(value) for value in policy.ordinary_value_identifiers
    )
    suffixes = ", ".join(
        f"{_markdown_code(value)} (remove the suffix)"
        for value in policy.numeric_suffix_fixits
    )
    lines = [
        "| Source policy | Canonical V1 values |",
        "| --- | --- |",
        f"| Active type spellings | {active} |",
        f"| Reserved retired numeric type spellings | {retired} |",
        f"| Ordinary value/function identifier examples | {ordinary} |",
        f"| Retired literal suffixes with safe fix-its | {suffixes} |",
        "",
        "| Source type | Rust nominal type | Pointer ID | Schema name | Schema hash |",
        "| --- | --- | --- | --- | --- |",
    ]
    for schema in policy.numeric_schemas:
        lines.append(
            f"| `{schema.source_type}` | `{schema.rust_type}` | "
            f"`0x{schema.pointer_id:04x}` | `{schema.schema_name}` | "
            f"`{schema.schema_hash}` |"
        )
    unassigned = ", ".join(
        f"`0x{pointer:04x}`" for pointer in policy.unassigned_pointers
    )
    lines.extend(
        [
            "",
            f"{unassigned} is unassigned and rejected as unknown; it is not an ABI tombstone.",
        ]
    )
    return "\n".join(lines)


def _regex_escape(value: str) -> str:
    return re.sub(r"([\\.^$|?*+()[\]{}])", r"\\\1", value)


def _keyword_pattern(grammar: LexicalGrammar) -> str:
    alternatives = "|".join(_regex_escape(value) for value, _ in grammar.keywords)
    return rf"(?<![\p{{L}}\p{{N}}_])(?:{alternatives})(?![\p{{L}}\p{{N}}_])"


def _operator_pattern(grammar: LexicalGrammar) -> str:
    sorted_operators = sorted(
        (value for value, _ in grammar.operators),
        key=lambda value: (-len(value), value),
    )
    return "(?:" + "|".join(_regex_escape(value) for value in sorted_operators) + ")"


def _alternation(values: Sequence[str]) -> str:
    return r"\b(?:" + "|".join(_regex_escape(value) for value in values) + r")\b"


def _textmate_entry(name: str, match: str, scope: str) -> str:
    raw = json.dumps(
        {name: {"patterns": [{"match": match, "name": scope}]}},
        ensure_ascii=False,
        indent=2,
    )
    lines = [f"  {line}" for line in raw.splitlines()[1:-1]]
    lines[-1] += ","
    return "\n".join(lines)


def _textmate_markers(name: str, *, final: bool = False) -> tuple[str, str]:
    normalized = re.sub(r"[^A-Za-z0-9]+", "_", name).upper()
    start = f'    "__BEGIN_GENERATED_KOTODAMA_V1_{normalized}__": {{}},'
    suffix = "" if final else ","
    end = f'    "__END_GENERATED_KOTODAMA_V1_{normalized}__": {{}}{suffix}'
    return start, end


def _retired_suffix_pattern(policy: Policy) -> str:
    suffixes = "|".join(_regex_escape(value) for value in policy.numeric_suffix_fixits)
    numeric = (
        r"(?:0[xX][0-9A-Fa-f_]+|0[bB][01_]+|"
        r"\d(?:[\d_]*\d)?(?:\.\d(?:[\d_]*\d)?)?"
        r"(?:[eE][+-]?\d(?:[\d_]*\d)?)?)"
    )
    return rf"(?<![A-Za-z0-9_]){numeric}(?:{suffixes})\b"


def render_textmate(
    text: str, policy: Policy, grammar: LexicalGrammar, *, path: Path
) -> str:
    entries = [
        (
            "sumVariants",
            _alternation(policy.sum_paths),
            "support.constant.variant.kotodama",
            False,
        ),
        (
            "roundingVariants",
            _alternation(policy.rounding_paths),
            "support.constant.variant.rounding.kotodama",
            False,
        ),
        (
            "memberCalls",
            rf"(?<=\.)(?:{'|'.join(map(_regex_escape, policy.editor_member_calls))})(?=\s*\()",
            "support.function.method.kotodama",
            False,
        ),
        (
            "retiredNumericSuffixes",
            _retired_suffix_pattern(policy),
            "invalid.deprecated.numeric.suffix.kotodama",
            False,
        ),
        (
            "types",
            _alternation((*policy.active_source_types, "Rounding")),
            "storage.type.kotodama",
            False,
        ),
        (
            "keywords",
            _keyword_pattern(grammar),
            "keyword.control.kotodama",
            False,
        ),
        (
            "operators",
            _operator_pattern(grammar),
            "keyword.operator.kotodama",
            True,
        ),
    ]
    rendered = text
    for name, match, scope, final in entries:
        start, end = _textmate_markers(name, final=final)
        rendered = _replace_generated(
            rendered,
            start,
            end,
            _textmate_entry(name, match, scope),
            path=path,
        )
    try:
        parsed = json.loads(rendered)
    except json.JSONDecodeError as error:
        raise GenerationError(
            f"{path}: generated TextMate JSON is invalid: {error}"
        ) from error
    includes = [
        pattern.get("include")
        for pattern in parsed.get("patterns", [])
        if isinstance(pattern, dict)
    ]
    if includes.count("#retiredNumericSuffixes") != 1:
        raise GenerationError(
            f"{path}: top-level patterns must include #retiredNumericSuffixes exactly once"
        )
    return rendered


def _js_policy(policy: Policy) -> str:
    lines = ["const SCHEMAS = Object.freeze({"]
    for schema in policy.numeric_schemas:
        lines.extend(
            [
                f"  {schema.source_type}: Object.freeze({{",
                f'    name: "{schema.schema_name}",',
                f'    hash: "{schema.schema_hash}",',
                f"    pointerType: 0x{schema.pointer_id:04x},",
                f"    scaled: {str(schema.scaled).lower()},",
                "  }),",
            ]
        )
    lines.extend(
        [
            "});",
            "",
            f"const NUMERIC_V1_MIN_KNOWN_POINTER_TYPE = 0x{policy.first_known_pointer:04x};",
            f"const NUMERIC_V1_MAX_ASSIGNED_POINTER_TYPE = 0x{policy.last_assigned_pointer:04x};",
        ]
    )
    return "\n".join(lines)


def _typescript_policy(policy: Policy) -> str:
    lines = ["  readonly schemas: {"]
    for schema in policy.numeric_schemas:
        lines.extend(
            [
                f"    readonly {schema.source_type}: {{",
                f'      readonly name: "{schema.schema_name}";',
                "      readonly hash: string;",
                f"      readonly pointerType: 0x{schema.pointer_id:04x};",
                f"      readonly scaled: {str(schema.scaled).lower()};",
                "    };",
            ]
        )
    lines.append("  };")
    return "\n".join(lines)


def _python_policy(policy: Policy) -> str:
    lines = [
        "NUMERIC_V1_SCHEMAS: Mapping[str, NumericV1Schema] = MappingProxyType(",
        "    {",
    ]
    for schema in policy.numeric_schemas:
        lines.extend(
            [
                f'        "{schema.source_type}": NumericV1Schema(',
                f'            "{schema.schema_name}",',
                f'            bytes.fromhex("{schema.schema_hash}"),',
                f"            0x{schema.pointer_id:04X},",
                f"            {schema.scaled},",
                "        ),",
            ]
        )
    lines.extend(
        [
            "    }",
            ")",
            '"""Canonical schema metadata keyed by source type name."""',
            "",
            f"_NUMERIC_V1_MIN_KNOWN_POINTER_TYPE = 0x{policy.first_known_pointer:04X}",
            f"_NUMERIC_V1_MAX_ASSIGNED_POINTER_TYPE = 0x{policy.last_assigned_pointer:04X}",
        ]
    )
    return "\n".join(lines)


def _kotlin_policy(policy: Policy) -> str:
    lines = [
        "private enum class NumericKind(",
        "    val schemaHash: ByteArray,",
        "    val pointerType: Int,",
        "    val scaled: Boolean,",
        ") {",
    ]
    for schema in policy.numeric_schemas:
        lines.append(
            f'    {schema.rust_type.upper()}("{schema.schema_hash}".hexBytes(), '
            f"0x{schema.pointer_id:04X}, {str(schema.scaled).lower()}),"
        )
    lines.extend(
        [
            "}",
            "",
            f"private const val MIN_KNOWN_POINTER_TYPE = 0x{policy.first_known_pointer:04X}",
            f"private const val MAX_ASSIGNED_POINTER_TYPE = 0x{policy.last_assigned_pointer:04X}",
        ]
    )
    return "\n".join(lines)


def _java_policy(policy: Policy) -> str:
    lines = ["  private enum Kind {"]
    for index, schema in enumerate(policy.numeric_schemas):
        ending = ";" if index == len(policy.numeric_schemas) - 1 else ","
        lines.append(
            f'    {schema.rust_type.upper()}("{schema.schema_hash}", '
            f"0x{schema.pointer_id:04X}, {str(schema.scaled).lower()}){ending}"
        )
    lines.extend(
        [
            "",
            "    final byte[] schemaHash;",
            "    final int pointerType;",
            "    final boolean scaled;",
            "",
            "    Kind(final String schemaHash, final int pointerType, final boolean scaled) {",
            "      this.schemaHash = hex(schemaHash);",
            "      this.pointerType = pointerType;",
            "      this.scaled = scaled;",
            "    }",
            "  }",
            "",
            f"  private static final int MIN_KNOWN_POINTER_TYPE = 0x{policy.first_known_pointer:04X};",
            f"  private static final int MAX_ASSIGNED_POINTER_TYPE = 0x{policy.last_assigned_pointer:04X};",
        ]
    )
    return "\n".join(lines)


def _swift_policy(policy: Policy) -> str:
    lines = ["private enum NumericV1Kind {"]
    lines.extend(f"    case {schema.source_type}" for schema in policy.numeric_schemas)
    lines.extend(["", "    var schemaName: String {", "        switch self {"])
    lines.extend(
        f'        case .{schema.source_type}: return "{schema.schema_name}"'
        for schema in policy.numeric_schemas
    )
    lines.extend(
        [
            "        }",
            "    }",
            "",
            "    var schemaHash: [UInt8] {",
            "        switch self {",
        ]
    )
    lines.extend(
        f'        case .{schema.source_type}: return NumericV1Internal.hex("{schema.schema_hash}")'
        for schema in policy.numeric_schemas
    )
    lines.extend(
        [
            "        }",
            "    }",
            "",
            "    var pointerType: UInt16 {",
            "        switch self {",
        ]
    )
    lines.extend(
        f"        case .{schema.source_type}: return 0x{schema.pointer_id:04X}"
        for schema in policy.numeric_schemas
    )
    lines.extend(
        [
            "        }",
            "    }",
            "",
            "    var isScaled: Bool {",
            "        switch self {",
        ]
    )
    lines.extend(
        f"        case .{schema.source_type}: return {str(schema.scaled).lower()}"
        for schema in policy.numeric_schemas
    )
    lines.extend(
        [
            "        }",
            "    }",
            "}",
            "",
            f"private let numericV1MinKnownPointerType: UInt16 = 0x{policy.first_known_pointer:04X}",
            f"private let numericV1MaxAssignedPointerType: UInt16 = 0x{policy.last_assigned_pointer:04X}",
        ]
    )
    return "\n".join(lines)


def _csharp_policy(policy: Policy) -> str:
    lines: list[str] = []
    for schema in policy.numeric_schemas:
        lines.extend(
            [
                f"    private static readonly NumericKind {schema.rust_type}Kind = new(",
                f'        Convert.FromHexString("{schema.schema_hash}"),',
                f"        0x{schema.pointer_id:04X},",
                f"        {str(schema.scaled).lower()});",
                "",
            ]
        )
    lines.extend(
        [
            f"    private const ushort MinKnownPointerType = 0x{policy.first_known_pointer:04X};",
            f"    private const ushort MaxAssignedPointerType = 0x{policy.last_assigned_pointer:04X};",
        ]
    )
    return "\n".join(lines)


def _declaration_names(policy: Policy) -> tuple[str, ...]:
    return (*policy.active_source_types, *policy.declaration_reserved_extras)


def _quoted_rows(values: Sequence[str], indentation: str) -> str:
    return "\n".join(
        f"{indentation}{json.dumps(value, ensure_ascii=False)}," for value in values
    )


def _javascript_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n".join(
        [
            "/** Canonical Kotodama V1 lexical keywords generated from `grammar/v1.lex`. */",
            "export const KOTODAMA_V1_KEYWORDS = Object.freeze([",
            _quoted_rows(keywords, "  "),
            "]);",
            "",
            "/** Names reserved for non-type source declarations. */",
            "export const KOTODAMA_V1_DECLARATION_RESERVED = Object.freeze([",
            _quoted_rows(_declaration_names(policy), "  "),
            "]);",
            "",
            "/** Retired numeric spellings reserved only for types and source units. */",
            "export const KOTODAMA_V1_RETIRED_TYPE_NAMES = Object.freeze([",
            _quoted_rows(policy.retired_numeric_type_spellings, "  "),
            "]);",
            "",
            "const KEYWORD_SET = new Set(KOTODAMA_V1_KEYWORDS);",
            "const DECLARATION_RESERVED_SET = new Set(KOTODAMA_V1_DECLARATION_RESERVED);",
            "const RETIRED_TYPE_SET = new Set(KOTODAMA_V1_RETIRED_TYPE_NAMES);",
        ]
    )


def _python_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    def frozen(name: str, values: Sequence[str]) -> str:
        return "\n".join(
            [f"{name} = frozenset(", "    {", _quoted_rows(values, "        "), "    }", ")"]
        )

    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n\n".join(
        [
            frozen("_KOTODAMA_RESERVED_IDENTIFIERS", keywords),
            frozen(
                "_KOTODAMA_RESERVED_DECLARATION_IDENTIFIERS",
                _declaration_names(policy),
            ),
            frozen(
                "_KOTODAMA_RETIRED_NUMERIC_TYPE_NAMES",
                policy.retired_numeric_type_spellings,
            ),
        ]
    )


def _kotlin_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    def kotlin_set(name: str, values: Sequence[str]) -> str:
        return "\n".join(
            [f"    private val {name} = setOf(", _quoted_rows(values, "        "), "    )"]
        )

    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n".join(
        [
            kotlin_set("reservedIdentifiers", keywords),
            kotlin_set("reservedDeclarationNames", _declaration_names(policy)),
            kotlin_set("retiredNumericTypeNames", policy.retired_numeric_type_spellings),
        ]
    )


def _java_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    def java_set(name: str, values: Sequence[str]) -> str:
        rows = "\n".join(
            f"          {json.dumps(value, ensure_ascii=False)}{',' if i + 1 < len(values) else ''}"
            for i, value in enumerate(values)
        )
        return f"  private static final Set<String> {name} =\n      set(\n{rows});"

    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n".join(
        [
            java_set("RESERVED_IDENTIFIERS", keywords),
            java_set("RESERVED_DECLARATION_NAMES", _declaration_names(policy)),
            java_set("RETIRED_NUMERIC_TYPE_NAMES", policy.retired_numeric_type_spellings),
        ]
    )


def _swift_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    def swift_set(name: str, values: Sequence[str]) -> str:
        return "\n".join(
            [f"    private static let {name}: Set<String> = [", _quoted_rows(values, "        "), "    ]"]
        )

    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n".join(
        [
            swift_set("reservedIdentifiers", keywords),
            swift_set("reservedDeclarationIdentifiers", _declaration_names(policy)),
            swift_set("retiredNumericTypeNames", policy.retired_numeric_type_spellings),
        ]
    )


def _csharp_validator_policy(policy: Policy, grammar: LexicalGrammar) -> str:
    def csharp_set(name: str, values: Sequence[str]) -> str:
        return "\n".join(
            [
                f"    private static readonly HashSet<string> {name} = new(StringComparer.Ordinal)",
                "    {",
                _quoted_rows(values, "        "),
                "    };",
            ]
        )

    keywords = tuple(spelling for spelling, _ in grammar.keywords)
    return "\n".join(
        [
            csharp_set("Keywords", keywords),
            csharp_set("ReservedDeclarationNames", _declaration_names(policy)),
            csharp_set("RetiredNumericTypeNames", policy.retired_numeric_type_spellings),
        ]
    )


def _replace_sdk(text: str, language: str, body: str, path: Path) -> str:
    prefix = "#" if language == "python" else "//"
    indentation = {
        "typescript": "  ",
        "java": "  ",
        "csharp": "    ",
    }.get(language, "")
    return _replace_generated(
        text,
        f"{indentation}{prefix} BEGIN GENERATED: {MARKER_NAME}",
        f"{indentation}{prefix} END GENERATED: {MARKER_NAME}",
        body,
        path=path,
    )


def _validate_rust_abi_sources(root: Path, policy: Policy) -> None:
    pointer_path = root / "crates/ivm_abi/src/pointer_abi.rs"
    pointer_text = _read_text(pointer_path)
    for schema in policy.numeric_schemas:
        declaration = f"{schema.rust_type} = 0x{schema.pointer_id:04X},"
        if declaration not in pointer_text:
            raise GenerationError(f"{pointer_path}: missing canonical {declaration}")
    if re.search(r"\b0x0013\s*=>\s*Some", pointer_text):
        raise GenerationError(f"{pointer_path}: 0x0013 must remain unassigned")

    numeric_path = root / "crates/iroha_primitives/src/numeric_abi.rs"
    numeric_text = _read_text(numeric_path)
    for schema in policy.numeric_schemas:
        prefix = schema.source_type.upper()
        name_line = f'pub const {prefix}_SCHEMA_NAME_V1: &str = "{schema.schema_name}";'
        if name_line not in numeric_text:
            raise GenerationError(f"{numeric_path}: missing canonical {name_line}")
        pattern = re.compile(
            rf"pub const {prefix}_SCHEMA_HASH_V1: \[u8; 16\] = \[(?P<body>.*?)\];",
            re.DOTALL,
        )
        match = pattern.search(numeric_text)
        if match is None:
            raise GenerationError(f"{numeric_path}: missing {prefix}_SCHEMA_HASH_V1")
        encoded = "".join(
            f"{int(value, 16):02x}"
            for value in re.findall(r"0x([0-9a-fA-F]{2})", match.group("body"))
        )
        if encoded != schema.schema_hash:
            raise GenerationError(
                f"{numeric_path}: {prefix}_SCHEMA_HASH_V1 differs from policy"
            )


def render_outputs(root: Path, policy: Policy) -> dict[Path, str]:
    """Render every generated file without writing it."""

    grammar = load_lexical_grammar(root, policy)
    _validate_rust_abi_sources(root, policy)
    outputs: dict[Path, str] = {}

    path = SEMANTIC_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        *SEMANTIC_MARKERS,
        render_semantic_policy(policy),
        path=path,
    )

    path = GRAMMAR_DOC_PATH
    docs = _read_text(root / path)
    docs = _replace_generated(
        docs, *KEYWORD_DOC_MARKERS, render_keyword_docs(grammar), path=path
    )
    docs = _replace_generated(
        docs, *OPERATOR_DOC_MARKERS, render_operator_docs(grammar), path=path
    )
    docs = _replace_generated(
        docs, *SOURCE_DOC_MARKERS, render_source_policy_docs(policy), path=path
    )
    outputs[path] = docs

    path = TEXTMATE_PATH
    outputs[path] = render_textmate(_read_text(root / path), policy, grammar, path=path)

    javascript_policy = _js_policy(policy)
    for path in JAVASCRIPT_PATHS:
        outputs[path] = _replace_sdk(
            _read_text(root / path), "javascript", javascript_policy, path
        )

    path = TYPESCRIPT_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "typescript", _typescript_policy(policy), path
    )
    path = PYTHON_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "python", _python_policy(policy), path
    )
    path = KOTLIN_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "kotlin", _kotlin_policy(policy), path
    )
    path = JAVA_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "java", _java_policy(policy), path
    )
    path = SWIFT_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "swift", _swift_policy(policy), path
    )
    path = CSHARP_PATH
    outputs[path] = _replace_sdk(
        _read_text(root / path), "csharp", _csharp_policy(policy), path
    )

    for path in JAVASCRIPT_IDENTIFIER_PATHS:
        outputs[path] = _replace_generated(
            _read_text(root / path),
            f"// BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
            f"// END GENERATED: {VALIDATOR_MARKER_NAME}",
            _javascript_validator_policy(policy, grammar),
            path=path,
        )
    path = PYTHON_MANIFEST_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        f"# BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
        f"# END GENERATED: {VALIDATOR_MARKER_NAME}",
        _python_validator_policy(policy, grammar),
        path=path,
    )
    path = KOTLIN_MANIFEST_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        f"    // BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
        f"    // END GENERATED: {VALIDATOR_MARKER_NAME}",
        _kotlin_validator_policy(policy, grammar),
        path=path,
    )
    path = JAVA_MANIFEST_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        f"  // BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
        f"  // END GENERATED: {VALIDATOR_MARKER_NAME}",
        _java_validator_policy(policy, grammar),
        path=path,
    )
    path = SWIFT_MANIFEST_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        f"    // BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
        f"    // END GENERATED: {VALIDATOR_MARKER_NAME}",
        _swift_validator_policy(policy, grammar),
        path=path,
    )
    path = CSHARP_MANIFEST_PATH
    outputs[path] = _replace_generated(
        _read_text(root / path),
        f"    // BEGIN GENERATED: {VALIDATOR_MARKER_NAME}",
        f"    // END GENERATED: {VALIDATOR_MARKER_NAME}",
        _csharp_validator_policy(policy, grammar),
        path=path,
    )

    if tuple(outputs) != GENERATED_TARGETS:
        raise GenerationError("internal generated target inventory drift")
    return outputs


def _atomic_write(path: Path, content: str) -> None:
    try:
        mode = stat.S_IMODE(path.stat().st_mode)
    except OSError as error:
        raise GenerationError(f"failed to stat {path}: {error}") from error
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as handle:
            handle.write(content)
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    except OSError as error:
        raise GenerationError(f"failed to write {path}: {error}") from error
    finally:
        temporary.unlink(missing_ok=True)


def apply_outputs(root: Path, outputs: Mapping[Path, str], *, check: bool) -> int:
    """Check or atomically write rendered outputs; return the drift count."""

    changed = 0
    for relative in GENERATED_TARGETS:
        expected = outputs[relative]
        path = root / relative
        actual = _read_text(path)
        if actual == expected:
            continue
        changed += 1
        if check:
            diff = difflib.unified_diff(
                actual.splitlines(),
                expected.splitlines(),
                fromfile=str(relative),
                tofile=f"{relative} (generated)",
                lineterm="",
            )
            print("\n".join(diff), file=sys.stderr)
        else:
            _atomic_write(path, expected)
            print(f"updated {relative}")
    return changed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check",
        action="store_true",
        help="verify generated files are current (the default)",
    )
    mode.add_argument(
        "--write",
        action="store_true",
        help="rewrite only marker-delimited generated regions",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=REPOSITORY_ROOT,
        help="repository root (primarily for isolated tests)",
    )
    parser.add_argument(
        "--policy",
        type=Path,
        default=DEFAULT_POLICY,
        help="repository-relative canonical policy descriptor",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    root = args.root.resolve()
    try:
        policy = load_policy(root, args.policy)
        outputs = render_outputs(root, policy)
        changed = apply_outputs(root, outputs, check=not args.write)
    except GenerationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if not args.write and changed:
        print(
            f"{changed} of {len(GENERATED_TARGETS)} generated files are stale; "
            "run scripts/regenerate_kotodama_syntax.py --write",
            file=sys.stderr,
        )
        return 1
    if args.write:
        print(f"updated {changed} of {len(GENERATED_TARGETS)} generated files")
    else:
        print(f"checked {len(GENERATED_TARGETS)} generated files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
