#!/usr/bin/env python3
"""Fail-closed guard for the Norito columnar/derive helper compaction.

The guard authenticates both indexed preimages, preserves public/test/type and
descriptor inventories, pins the readable canonical-helper surfaces, and
rejects line packing, callback/body DSLs, or source relocation.  It uses only
the Python standard library so it can run before Cargo is available.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import unittest
from functools import lru_cache
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
COLUMNAR_PATH = ROOT / "crates/norito/src/columnar.rs"
DERIVE_PATH = ROOT / "crates/norito_derive/src/lib.rs"
ATTRIBUTE_HELPERS_PATH = ROOT / "crates/norito_derive/src/attribute_helpers.rs"

COLUMNAR_PREIMAGE_BLOB = "aeaaeea26f4d315df2f5a6ce092c5c0570f9fa4c"
DERIVE_PREIMAGE_BLOB = "a933a74dda41002f4d537671fddd77f2935b4efe"
COLUMNAR_PREIMAGE_SHA256 = (
    "6ca38e16a814e0ef69248279f08c59d06462e88826ea2876cc56df8a123be833"
)
DERIVE_PREIMAGE_SHA256 = (
    "20fde3fefb438100e1cc036fdaf6f04f2c8c2c0e95c578b193c6276301b733f4"
)
COLUMNAR_PREIMAGE_LINES = 6_531
DERIVE_PREIMAGE_LINES = 5_869
OPENING_RUST_LINES = COLUMNAR_PREIMAGE_LINES + DERIVE_PREIMAGE_LINES
MINIMUM_RUST_LINE_REDUCTION = 1_200

COLUMNAR_LINE_CEILING = 5_644
DERIVE_LINE_CEILING = 5_129
COLUMNAR_MAX_LINE_LENGTH = 120
DERIVE_MAX_LINE_LENGTH = 180
COLUMNAR_SHA256 = "640e4ed77080e46d61128dae3342ccfae5cee9cc08b3e5ec26ba05c8750ef0e9"
DERIVE_SHA256 = "099c821fcf75c8a93937444bb13d3c7a3b35685e24ca5ccdac6e7990f314fa20"
ATTRIBUTE_HELPERS_SHA256 = (
    "c546dd7ed13e22c26b398a45cb5779f6f5e97fdd6c1e1ba7a0b1887f3f364f4c"
)

COLUMNAR_CFG_SURFACE_SHA256 = (
    "43dbc7bec7e7e01e1bafbfbec0833b8e05279c30dc0d389f6df34edc9976fc0b"
)
DERIVE_CFG_SURFACE_SHA256 = (
    "241b1af584e62103537ee551411a98d0c2a8f00ca17fb93a47b7b46ce952ffcc"
)
COLUMNAR_HELPER_SURFACE_SHA256 = (
    "ae32ff076c39cc6c721ef8f244adeab11c0cbee0c6aca67ed9da293003bfd9bf"
)
DERIVE_HELPER_SURFACE_SHA256 = (
    "75c1f4dcce25d13f507d29d9d77cd4d492f96075afe8fef439a1eb2e4c12884d"
)

COLUMNAR_HELPERS = (
    "read_ncb_header",
    "take_bitset_tail",
    "optional_column_prefix",
    "bit_at",
    "decode_u32_column",
    "decode_offset_names",
    "decode_dict_names",
    "tagged_payload",
    "finish_two_pass",
    "finish_selection",
    "split_tagged_payload",
    "parse_aos_u64_var_bool",
    "parse_aos_u64_var_u32_bool",
    "validated_str",
    "write_id_column",
    "write_flag_column",
    "write_var_u64",
)
DERIVE_HELPERS = (
    "needs_packed_size_with_attrs",
    "struct_fields",
    "active_struct_fields",
    "struct_has_flatten",
    "struct_has_signature_like",
    "packed_field_bitset_from",
    "packed_bit_positions",
    "derive_struct_len_body",
    "generic_arguments",
    "packed_serialize_parts",
    "struct_serialize_calls",
    "packed_size_headers",
    "derive_decode_from_slice_impl",
    "decode_from_archived_body",
    "sequential_deserialize_value",
    "json_flatten_parts",
    "type_ident",
    "single_type_argument",
    "fast_json_assign_field",
    "fast_json_parser_field",
    "fast_json_vec_field",
    "fast_json_option_field",
    "fast_json_field_parser",
    "binary_field_value_with_default",
)

COLUMNAR_TESTS = (
    "ncb_row_count_prefix_rejects_truncated_inputs",
    "ncb_row_count_prefix_rejects_disproportionate_allocation_without_limit_scope",
    "ncb_row_count_views_reject_truncated_headers",
    "should_use_columnar_respects_heuristics_threshold",
    "small_smart_n_matches_heuristics",
    "u32_delta_toggle_respects_name_flag",
    "u32_delta_toggle_respects_bytes_flag",
)
DERIVE_TESTS = (
    "byte_array_length_classifier_rejects_other_arrays",
    "explicit_discriminants_drive_implicit_successors",
    "codec_index_can_override_an_implicit_rust_discriminant",
    "duplicate_effective_index_is_rejected",
    "codec_index_must_match_explicit_discriminant",
    "non_literal_discriminant_is_rejected",
    "deny_unknown_fields_attribute_is_parsed",
    "deny_unknown_fields_after_attributes_owned_by_other_derives_is_parsed",
    "schema_name_and_deny_unknown_fields_are_combined",
    "duplicate_deny_unknown_fields_attribute_is_rejected",
    "deny_unknown_fields_value_is_rejected",
    "unknown_container_attribute_is_rejected",
    "attributes_owned_by_other_norito_derives_are_accepted",
    "duplicate_shared_container_attribute_is_rejected",
)

DERIVE_LITERAL_ADDITIONS = {
    '"__e"',
    '"__h"',
    '"encoded_len_exact"',
    '"encoded_len_hint"',
    '"named fields must have identifiers"',
}

FORBIDDEN = re.compile(
    r"dyn\s+Fn|FnOnce|\bfn\s*\(|\$(?:body|setup|action)|"
    r"\b(?:struct|enum|type)\s+(?:Action|Step)\b|"
    r"include_(?:str|bytes)!|macro_rules!"
)
FUNCTION_ITEM = re.compile(
    r"(?m)^[ \t]*(?:(pub(?:\s*\([^\n)]*\))?)\s+)?"
    r"(?:(?:const|async|unsafe)\s+)*fn\s+([A-Za-z_]\w*)"
)
PUBLIC_TYPE = re.compile(
    r"(?m)^[ \t]*pub(?:\s*\([^\n)]*\))?\s+"
    r"(struct|enum|type|trait|union|const|static)\s+([A-Za-z_]\w*)"
)
CONST_ITEM = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\s*\([^\n)]*\))?\s+)?const\s+([A-Za-z_]\w*)"
)
RAW_STRING_START = re.compile(r'(?:b?r)(#*)"')


class GuardError(AssertionError):
    """The compacted sources no longer match their authenticated contract."""


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _json_sha256(value: object) -> str:
    payload = json.dumps(value, separators=(",", ":"), ensure_ascii=True)
    return _sha256(payload)


def _blob(blob: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "cat-file", "blob", blob],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
    except subprocess.CalledProcessError as error:
        raise GuardError(f"authenticated preimage {blob} is unavailable") from error


def _normalise(source: str) -> str:
    return re.sub(r"\s+", " ", source).strip()


def _compact(source: str) -> str:
    return re.sub(r"\s+", "", source)


def _skip_quoted(source: str, start: int) -> int:
    raw = RAW_STRING_START.match(source, start)
    if raw:
        terminator = '"' + raw.group(1)
        end = source.find(terminator, raw.end())
        if end < 0:
            raise GuardError("unterminated Rust raw string")
        return end + len(terminator)
    quote_start = start + (1 if source.startswith('b"', start) else 0)
    quote = source[quote_start]
    cursor = quote_start + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == quote:
            return cursor + 1
        else:
            cursor += 1
    raise GuardError("unterminated Rust string literal")


def _matching_delimiter(source: str, opening: int) -> int:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    cursor = opening
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise GuardError("unterminated Rust block comment")
            continue
        if source[cursor] == '"' or source.startswith('b"', cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        if RAW_STRING_START.match(source, cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            close = cursor + 2 if source[cursor + 1] != "\\" else cursor + 3
            if close < len(source) and source[close] == "'":
                cursor = close + 1
                continue
        character = source[cursor]
        if character in pairs:
            stack.append(character)
        elif character in pairs.values():
            if not stack or pairs[stack[-1]] != character:
                raise GuardError(f"unbalanced Rust delimiter at byte {cursor}")
            stack.pop()
            if not stack:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust delimiter")


@lru_cache(maxsize=64)
def _mask_non_code(source: str) -> str:
    masked = list(source)

    def blank(start: int, end: int) -> None:
        for index in range(start, end):
            if masked[index] != "\n":
                masked[index] = " "

    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end < 0 else end
            blank(cursor, end)
            cursor = end
            continue
        if source.startswith("/*", cursor):
            depth = 1
            end = cursor + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise GuardError("unterminated Rust block comment")
            blank(cursor, end)
            cursor = end
            continue
        quoted = (
            source[cursor] == '"'
            or source.startswith('b"', cursor)
            or RAW_STRING_START.match(source, cursor) is not None
        )
        if quoted:
            end = _skip_quoted(source, cursor)
            blank(cursor, end)
            cursor = end
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            if source[cursor + 1] == "\\":
                end = source.find("'", cursor + 2)
                if 0 < end - cursor <= 12:
                    blank(cursor, end + 1)
                    cursor = end + 1
                    continue
            elif source[cursor + 2] == "'":
                blank(cursor, cursor + 3)
                cursor += 3
                continue
        cursor += 1
    return "".join(masked)


@lru_cache(maxsize=64)
def _validate_delimiters(source: str) -> None:
    masked = _mask_non_code(source)
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[tuple[str, int]] = []
    for index, character in enumerate(masked):
        if character in pairs:
            stack.append((character, index))
        elif character in pairs.values():
            if not stack or pairs[stack[-1][0]] != character:
                raise GuardError(f"unbalanced Rust delimiter at byte {index}")
            stack.pop()
    if stack:
        raise GuardError(f"unterminated Rust delimiter at byte {stack[-1][1]}")


def _attributes_before(source: str, masked: str, start: int) -> tuple[str, ...]:
    attributes: list[str] = []
    cursor = start
    while True:
        while cursor and masked[cursor - 1].isspace():
            cursor -= 1
        if not cursor or masked[cursor - 1] != "]":
            break
        end = cursor
        depth = 0
        opening = cursor - 1
        while opening >= 0:
            if masked[opening] == "]":
                depth += 1
            elif masked[opening] == "[":
                depth -= 1
                if depth == 0:
                    break
            opening -= 1
        if opening <= 0 or masked[opening - 1] != "#":
            break
        beginning = opening - 1
        attributes.append(_compact(source[beginning:end]))
        cursor = beginning
    return tuple(reversed(attributes))


@lru_cache(maxsize=64)
def _function_records(source: str) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    masked = _mask_non_code(source)
    records = []
    for match in FUNCTION_ITEM.finditer(masked):
        opening = masked.find("{", match.end())
        if opening < 0:
            raise GuardError(f"missing body for function {match.group(2)}")
        records.append(
            (
                match.group(2),
                _compact(source[match.start() : opening]),
                _attributes_before(source, masked, match.start()),
            )
        )
    return tuple(records)


@lru_cache(maxsize=64)
def _public_function_inventory(
    source: str,
) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    masked = _mask_non_code(source)
    public = []
    for match in FUNCTION_ITEM.finditer(masked):
        if match.group(1) is None:
            continue
        opening = masked.find("{", match.end())
        if opening < 0:
            raise GuardError(f"missing body for public function {match.group(2)}")
        public.append(
            (
                match.group(2),
                _compact(source[match.start() : opening]),
                _attributes_before(source, masked, match.start()),
            )
        )
    return tuple(public)


@lru_cache(maxsize=64)
def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    tests = []
    for name, _header, attributes in _function_records(source):
        if "#[test]" in attributes:
            tests.append((name, attributes))
    return tuple(tests)


@lru_cache(maxsize=64)
def _public_type_inventory(
    source: str,
) -> tuple[tuple[str, str, str, tuple[str, ...]], ...]:
    masked = _mask_non_code(source)
    inventory = []
    for match in PUBLIC_TYPE.finditer(masked):
        endings = [
            ending
            for ending in (masked.find("{", match.end()), masked.find(";", match.end()))
            if ending >= 0
        ]
        if not endings:
            raise GuardError(f"missing end of public {match.group(1)} {match.group(2)}")
        end = min(endings)
        inventory.append(
            (
                match.group(1),
                match.group(2),
                _compact(source[match.start() : end]),
                _attributes_before(source, masked, match.start()),
            )
        )
    return tuple(inventory)


@lru_cache(maxsize=64)
def _constant_inventory(source: str) -> tuple[tuple[str, str], ...]:
    masked = _mask_non_code(source)
    inventory = []
    for match in CONST_ITEM.finditer(masked):
        end = masked.find(";", match.end())
        if end < 0:
            raise GuardError(f"missing end of constant {match.group(1)}")
        inventory.append((match.group(1), _compact(source[match.start() : end + 1])))
    return tuple(inventory)


@lru_cache(maxsize=64)
def _literal_tokens(source: str) -> frozenset[str]:
    literals: set[str] = set()
    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise GuardError("unterminated Rust block comment")
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            if source[cursor + 1] == "\\":
                end = source.find("'", cursor + 2)
                if 0 < end - cursor <= 12:
                    cursor = end + 1
                    continue
            elif source[cursor + 2] == "'":
                cursor += 3
                continue
        quoted = (
            source[cursor] == '"'
            or source.startswith('b"', cursor)
            or RAW_STRING_START.match(source, cursor) is not None
        )
        if quoted:
            end = _skip_quoted(source, cursor)
            literals.add(source[cursor:end])
            cursor = end
            continue
        cursor += 1
    return frozenset(literals)


@lru_cache(maxsize=64)
def _cfg_surface(source: str) -> tuple[str, ...]:
    attributes = []
    for match in re.finditer(r"#\s*\[\s*cfg\b", source):
        opening = source.find("[", match.start(), match.end())
        closing = _matching_delimiter(source, opening)
        attributes.append(_compact(source[match.start() : closing + 1]))
    return tuple(attributes)


def _named_function(source: str, name: str) -> str:
    match = re.search(
        rf"(?m)^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?"
        rf"fn\s+{re.escape(name)}(?:\s*<[^{{;\n]*>)?\s*\(",
        source,
    )
    if match is None:
        raise GuardError(f"missing canonical helper: {name}")
    opening = source.find("{", match.end())
    if opening < 0:
        raise GuardError(f"missing body for canonical helper: {name}")
    closing = _matching_delimiter(source, opening)
    return source[match.start() : closing + 1]


@lru_cache(maxsize=16)
def _helper_surface(source: str, names: tuple[str, ...]) -> tuple[tuple[str, str], ...]:
    return tuple((name, _normalise(_named_function(source, name))) for name in names)


def _require_count(source: str, token: str, expected: int, label: str) -> None:
    if source.count(token) != expected:
        raise GuardError(f"{label} changed")


def _validate(
    columnar: str,
    derive: str,
    attribute_helpers: str,
    columnar_preimage: str,
    derive_preimage: str,
    *,
    authenticate_final: bool,
) -> None:
    authenticated = (
        (
            columnar_preimage,
            COLUMNAR_PREIMAGE_SHA256,
            COLUMNAR_PREIMAGE_LINES,
            "columnar",
        ),
        (derive_preimage, DERIVE_PREIMAGE_SHA256, DERIVE_PREIMAGE_LINES, "derive"),
    )
    for preimage, digest, lines, label in authenticated:
        if _sha256(preimage) != digest or len(preimage.splitlines()) != lines:
            raise GuardError(f"authenticated {label} preimage changed")
    if _sha256(attribute_helpers) != ATTRIBUTE_HELPERS_SHA256:
        raise GuardError("current attribute-helper include changed")

    sources = (
        (
            columnar,
            COLUMNAR_LINE_CEILING,
            COLUMNAR_MAX_LINE_LENGTH,
            "columnar",
        ),
        (derive, DERIVE_LINE_CEILING, DERIVE_MAX_LINE_LENGTH, "derive"),
    )
    for source, ceiling, max_length, label in sources:
        if not source.endswith("\n"):
            raise GuardError(f"{label} source lost its final newline")
        _validate_delimiters(source)
        lines = source.splitlines()
        if len(lines) > ceiling:
            raise GuardError(f"{label} source exceeded its compacted line ceiling")
        if max(map(len, lines), default=0) > max_length:
            raise GuardError(f"{label} source appears line-packed")
        if FORBIDDEN.search(source):
            raise GuardError(f"{label} introduced a callback/body DSL or source relocation")
    final_lines = len(columnar.splitlines()) + len(derive.splitlines())
    if OPENING_RUST_LINES - final_lines < MINIMUM_RUST_LINE_REDUCTION:
        raise GuardError("Norito Rust reduction fell below 1,200 lines")
    if derive.count("FnMut") != 1 or "F: FnMut(String) -> String," not in derive:
        raise GuardError("the single pre-existing identifier-mapping closure changed")

    if authenticate_final:
        if _sha256(columnar) != COLUMNAR_SHA256:
            raise GuardError("authenticated compacted columnar source changed")
        if _sha256(derive) != DERIVE_SHA256:
            raise GuardError("authenticated compacted derive source changed")

    inventories = (
        (_public_function_inventory, "public function"),
        (_public_type_inventory, "public type"),
        (_constant_inventory, "descriptor/constant"),
        (_test_inventory, "test name/attribute/order"),
    )
    for inventory, label in inventories:
        if inventory(columnar) != inventory(columnar_preimage):
            raise GuardError(f"columnar {label} inventory changed")
        if inventory(derive) != inventory(derive_preimage):
            raise GuardError(f"derive {label} inventory changed")
    if tuple(name for name, _attrs in _test_inventory(columnar)) != COLUMNAR_TESTS:
        raise GuardError("columnar direct-test inventory changed")
    if tuple(name for name, _attrs in _test_inventory(derive)) != DERIVE_TESTS:
        raise GuardError("derive direct-test inventory changed")

    if _literal_tokens(columnar) != _literal_tokens(columnar_preimage):
        raise GuardError("columnar diagnostic/string literal set changed")
    derive_preimage_literals = _literal_tokens(derive_preimage)
    derive_literals = _literal_tokens(derive)
    if derive_preimage_literals - derive_literals:
        raise GuardError("derive lost a current diagnostic/string literal")
    if derive_literals - derive_preimage_literals != DERIVE_LITERAL_ADDITIONS:
        raise GuardError("derive introduced an unaudited diagnostic/string literal")

    includes = (
        tuple(re.findall(r'include!\(\s*"([^"]+)"\s*\)', columnar)),
        tuple(re.findall(r'include!\(\s*"([^"]+)"\s*\)', derive)),
    )
    if includes[0] != ("columnar_adaptive_test.rs",):
        raise GuardError("columnar include boundary changed")
    if includes[1] != (
        "attribute_helpers.rs",
        "tests/generic_bounds.rs",
        "tests/deserialize_codegen.rs",
    ):
        raise GuardError("derive include boundary changed")
    if tuple(re.findall(r'#\[path\s*=\s*"([^"]+)"\]', derive)) != tuple(
        re.findall(r'#\[path\s*=\s*"([^"]+)"\]', derive_preimage)
    ):
        raise GuardError("derive path-module boundary changed")
    if "fn consume_unknown_meta(" in derive or "fn u8_array_len(" in derive:
        raise GuardError("attribute-helper bodies moved back into derive lib.rs")

    cfg_surfaces = (
        (
            _cfg_surface(columnar),
            COLUMNAR_CFG_SURFACE_SHA256,
            37,
            "columnar feature cfg",
        ),
        (_cfg_surface(derive), DERIVE_CFG_SURFACE_SHA256, 13, "derive feature cfg"),
    )
    for surface, digest, count, label in cfg_surfaces:
        if len(surface) != count or _json_sha256(surface) != digest:
            raise GuardError(f"{label} surface changed")
    helper_surfaces = (
        (
            _helper_surface(columnar, COLUMNAR_HELPERS),
            COLUMNAR_HELPER_SURFACE_SHA256,
            "columnar canonical helper",
        ),
        (
            _helper_surface(derive, DERIVE_HELPERS),
            DERIVE_HELPER_SURFACE_SHA256,
            "derive canonical helper",
        ),
    )
    for surface, digest, label in helper_surfaces:
        if _json_sha256(surface) != digest:
            raise GuardError(f"{label} surface changed")

    compact_columnar = _compact(columnar)
    required_wire_anchors = (
        "let(tag,payload)=ifncb_len<aos_len{(ADAPTIVE_TAG_NCB,ncb)}else{(ADAPTIVE_TAG_AOS,aos)};",
        '#[cfg(feature="adaptive-telemetry")]telemetry::record_two_pass_times(_aos_ns,_ncb_ns);',
        '#[cfg(feature="adaptive-telemetry-log")]log_two_pass(_kind,tag,aos_len,ncb_len,_aos_ns,_ncb_ns);',
        '#[cfg(feature="simdutf8-validate")]{simdutf8::basic::from_utf8(bytes).map_err(|_|Error::InvalidUtf8)}',
        '#[cfg(not(feature="simdutf8-validate"))]{std::str::from_utf8(bytes).map_err(|_|Error::InvalidUtf8)}',
    )
    if any(anchor not in compact_columnar for anchor in required_wire_anchors):
        raise GuardError("columnar wire/telemetry/UTF-8 helper semantics changed")
    for kind in (
        "u64_str_bool",
        "u64_optstr_bool",
        "u64_optu32_bool",
        "u64_bytes_bool",
        "u64_u32_bool",
        "u64_str_u32_bool",
        "u64_bytes_u32_bool",
        "u64_enum_bool",
    ):
        _require_count(columnar, f'"{kind}"', 1, f"adaptive kind {kind}")
    for helper in (
        "encode_ncb_u64_str_u32_bool_force_u32_delta",
        "encode_ncb_u64_bytes_u32_bool_force_u32_delta",
    ):
        _require_count(columnar, f"fn {helper}", 1, f"current-only {helper}")

    derive_counts = (
        ("ensure_document_depth", 4, "document-depth guards"),
        ("try_reserve_exact", 2, "allocation guards"),
        ("serialize_to_writer_exact", 1, "exact writer route"),
        ("encoded_payload_len", 1, "exact payload length route"),
        ("write_len_prefixed_exact", 5, "length-prefixed exact routes"),
        ("fn write_json_to", 4, "bounded JSON writers"),
        ("unknown_field(key.as_str())", 3, "key-bearing unknown-field diagnostics"),
        ("u8_array_len(&field.field.ty).is_some()", 3, "typed struct u8 classifiers"),
        ("u8_array_len(&f.ty).is_some()", 2, "typed enum u8 classifiers"),
    )
    for token, expected, label in derive_counts:
        _require_count(derive, token, expected, label)
    if any(
        message in derive
        for message in (
            "unknown JSON field",
            "unknown JSON variant field",
            "unknown JSON enum envelope field",
        )
    ):
        raise GuardError("key-bearing JSON diagnostics were replaced with generic messages")
    if "syn::Type::Array(_) => u8_array_len(ty).map(|_| 0)" not in derive:
        raise GuardError("current u8-array fixed-size classifier changed")


class NoritoColumnarDeriveCompactionSourceTest(unittest.TestCase):
    """Exercise the source contract and deliberate fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.columnar = COLUMNAR_PATH.read_text(encoding="utf-8")
        cls.derive = DERIVE_PATH.read_text(encoding="utf-8")
        cls.attribute_helpers = ATTRIBUTE_HELPERS_PATH.read_text(encoding="utf-8")
        cls.columnar_preimage = _blob(COLUMNAR_PREIMAGE_BLOB)
        cls.derive_preimage = _blob(DERIVE_PREIMAGE_BLOB)

    def validate(
        self,
        columnar: str | None = None,
        derive: str | None = None,
        attribute_helpers: str | None = None,
        *,
        authenticate_final: bool = False,
    ) -> None:
        _validate(
            self.columnar if columnar is None else columnar,
            self.derive if derive is None else derive,
            self.attribute_helpers if attribute_helpers is None else attribute_helpers,
            self.columnar_preimage,
            self.derive_preimage,
            authenticate_final=authenticate_final,
        )

    def assert_mutation_rejected(self, **changes: str) -> None:
        with self.assertRaises(GuardError):
            self.validate(**changes)

    def test_compacted_source_contract(self) -> None:
        self.validate(authenticate_final=True)

    def test_descriptor_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            columnar=self.columnar.replace(
                "const DESC_U64_STR_BOOL: u8 = 0x13;",
                "const DESC_U64_STR_BOOL: u8 = 0x12;",
                1,
            )
        )

    def test_public_function_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            columnar=self.columnar.replace("pub fn materialize_ncb", "pub fn materialize_ncb_changed", 1)
        )

    def test_test_inventory_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace(
                "fn byte_array_length_classifier_rejects_other_arrays",
                "fn byte_array_length_classifier_accepts_other_arrays",
                1,
            )
        )

    def test_feature_cfg_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            columnar=self.columnar.replace(
                '#[cfg(feature = "adaptive-telemetry")]',
                '#[cfg(feature = "adaptive-telemetry-log")]',
                1,
            )
        )

    def test_depth_guard_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace("w.ensure_document_depth()?;", "w.skip_ws();", 1)
        )

    def test_allocation_guard_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace("try_reserve_exact", "reserve_exact", 1)
        )

    def test_json_diagnostic_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace(
                "norito::json::Error::unknown_field(key.as_str())",
                'norito::json::Error::Message("unknown JSON field".into())',
                1,
            )
        )

    def test_callback_dsl_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace("struct StructField<'a>", "struct Action<'a>", 1)
        )

    def test_include_relocation_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive.replace(
                'include!("attribute_helpers.rs");',
                'include!("canonical_body.rs");',
                1,
            )
        )

    def test_attribute_helper_mutation_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            attribute_helpers=self.attribute_helpers.replace('is_ident("u8")', 'is_ident("u16")', 1)
        )

    def test_delimiter_mutation_is_rejected(self) -> None:
        closing = self.columnar.rfind("}")
        self.assert_mutation_rejected(columnar=self.columnar[:closing] + self.columnar[closing + 1 :])

    def test_source_growth_is_rejected(self) -> None:
        growth = "\n// growth mutation" * (MINIMUM_RUST_LINE_REDUCTION + 1)
        self.assert_mutation_rejected(columnar=self.columnar + growth)

    def test_line_packing_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            derive=self.derive + "\n// " + "x" * DERIVE_MAX_LINE_LENGTH + "\n"
        )


if __name__ == "__main__":
    unittest.main()
