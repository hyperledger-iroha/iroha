#!/usr/bin/env python3
"""Check current Norito codec ownership contracts without historical source pins.

This stdlib source check recognizes scoped calls, guards, and protocol constants.
It is not a Rust parser or proof of behavior: source-sealed Rust runtime/codegen
suites, feature qualification, source budgets, and build provenance remain required.
Private helper names, line counts, full-file digests and test ordering are not policy.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

CORE = "crates/norito/src/core.rs"
ENCODER = "crates/norito/src/core/encoder.rs"
FIELDS = "crates/norito/src/core/encode_fields.rs"
FRAMES = "crates/norito/src/core/encode_frames.rs"
COLUMNAR = "crates/norito/src/columnar.rs"
DERIVE = "crates/norito_derive/src/lib.rs"
ATTRS = "crates/norito_derive/src/attribute_helpers.rs"
JSON_WRITER = "crates/norito_derive/src/json_write_bounded.rs"
IDENTITY = "crates/norito/src/schema/identity.rs"
IDENTITY_DERIVE = "crates/norito_derive/src/schema_identity.rs"
CODEGEN = "crates/norito_derive/src/tests/deserialize_codegen.rs"
COUNT_TESTS = "crates/norito/src/core/counting_tests.rs"
OWNERS = (CORE, ENCODER, FIELDS, FRAMES, COLUMNAR, DERIVE, ATTRS, JSON_WRITER, IDENTITY, IDENTITY_DERIVE)
SOURCE_FILES = (*OWNERS, CODEGEN, COUNT_TESTS)
RAW_STRING_START = re.compile(r'(?:b?r)(#*)"')

class ContractError(AssertionError):
    """A current, scoped codec ownership contract is missing."""


def require(condition: bool, diagnostic: str) -> None:
    if not condition:
        raise ContractError(diagnostic)


def compact(source: str) -> str:
    return re.sub(r"\s+", "", _mask_non_code(source))


def _skip_quoted(source: str, start: int) -> int:
    raw = RAW_STRING_START.match(source, start)
    if raw:
        terminator = '"' + raw.group(1)
        end = source.find(terminator, raw.end())
        if end < 0:
            raise ContractError("unterminated Rust raw string")
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
    raise ContractError("unterminated Rust string literal")


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
                raise ContractError("unterminated Rust block comment")
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


def closing_brace(masked: str, opening: int) -> int:
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    raise ContractError("source.unbalanced_body")


def production(source: str) -> str:
    """Exclude test-module bodies so a test cannot satisfy a production contract."""
    masked = _mask_non_code(source)
    result = list(source)
    pattern = r"(?m)^[ \t]*((?:#\[[^\n]*\]\s*)+)mod\s+\w+\s*\{"
    for match in re.finditer(pattern, source):
        if not re.search(r"\bcfg\b.*\btest\b", match.group(1)):
            continue
        opening = match.end() - 1
        if masked[opening] != "{":
            continue
        end = closing_brace(masked, opening)
        result[match.start():end + 1] = ["\n" if char == "\n" else " " for char in source[match.start():end + 1]]
    return "".join(result)


@dataclass(frozen=True)
class Function:
    """One lexical function region; offsets address its original source."""

    name: str
    start: int
    opening: int
    end: int
    source: str

    @property
    def code(self) -> str:
        return compact(self.source[self.opening + 1:self.end])

    @property
    def signature(self) -> str:
        return compact(self.source[self.start:self.opening])

    @property
    def raw_body(self) -> str:
        return self.source[self.opening + 1:self.end]


@lru_cache(maxsize=48)
def functions(source: str) -> tuple[Function, ...]:
    masked = _mask_non_code(source)
    result = []
    for match in re.finditer(r"\bfn\s+([A-Za-z_]\w*)\s*(?:<|\()", masked):
        opening = -1
        brackets = 0
        parentheses = 0
        for index in range(match.end() - 1, len(masked)):
            char = masked[index]
            brackets += (char == "[") - (char == "]")
            parentheses += (char == "(") - (char == ")")
            if char == "{" and brackets == 0 and parentheses == 0:
                opening = index
                break
            if char == ";" and brackets == 0 and parentheses == 0:
                break  # Trait declarations have no executable body.
        if opening < 0:
            continue
        end = closing_brace(masked, opening)
        result.append(Function(match.group(1), match.start(), opening, end, source))
    return tuple(result)


def operation(sources: dict[str, str], path: str, name: str) -> Function:
    matches = [item for item in functions(production(sources[path])) if item.name == name]
    require(len(matches) == 1, f"owner.operation:{path}::{name}")
    return matches[0]


def role(sources: dict[str, str], path: str, predicate, diagnostic: str) -> Function:
    matches = [item for item in functions(production(sources[path])) if predicate(item)]
    require(len(matches) == 1, diagnostic)
    return matches[0]


def reachable(sources: dict[str, str], roots: tuple[str, ...]) -> tuple[Function, ...]:
    """Follow uniquely named local codegen helpers from public derive entry points."""
    available: dict[str, list[Function]] = {}
    for path in (DERIVE, ATTRS, JSON_WRITER):
        for item in functions(production(sources[path])):
            available.setdefault(item.name, []).append(item)
    pending = [operation(sources, DERIVE, name) for name in roots]
    seen: dict[tuple[str, int], Function] = {}
    while pending:
        item = pending.pop()
        key = (item.name, item.start)
        if key in seen:
            continue
        seen[key] = item
        code = _mask_non_code(item.raw_body)
        for match in re.finditer(r"(?<![\w:.])([A-Za-z_]\w*)\s*(?:::<[^;{}]*?>)?\s*\(", code):
            if code[max(0, match.start() - 3):match.start()].strip() == "fn":
                continue
            candidates = available.get(match.group(1), ())
            if len(candidates) == 1:
                pending.append(candidates[0])
    return tuple(seen.values())


def has_literal_syntax(source: str, attribute: str) -> bool:
    # Literal feature names matter, but comment/string lookalikes do not count.
    masked = _mask_non_code(source)
    pattern = r"\s*".join(re.escape(char) for char in attribute)
    expected = compact(attribute)
    return any(re.sub(r"\s+", "", masked[match.start():match.end()]) == expected for match in re.finditer(pattern, source))


def validate_columnar(sources: dict[str, str]) -> None:
    code = _mask_non_code(production(sources[COLUMNAR]))
    for name, expected in COLUMNAR_WIRE_VALUES.items():
        match = re.search(rf"\bconst\s+{name}\s*:\s*u8\s*=\s*(0x[0-9A-Fa-f]+|[0-9]+)(?:u8)?\s*;", code)
        require(match is not None and int(match.group(1), 0) == expected, f"columnar.wire_value:{name}")
    selection = role(sources, COLUMNAR, lambda item: "ADAPTIVE_TAG_NCB,ncb" in item.code and "ADAPTIVE_TAG_AOS,aos" in item.code, "columnar.selection_owner")
    require("ifncb_len<aos_len{(ADAPTIVE_TAG_NCB,ncb)}else{(ADAPTIVE_TAG_AOS,aos)}" in selection.code, "columnar.aos_wins_equal_length")
    require(has_literal_syntax(selection.raw_body, '#[cfg(feature="adaptive-telemetry")]') and has_literal_syntax(selection.raw_body, '#[cfg(feature="adaptive-telemetry-log")]'), "columnar.telemetry_feature_isolation")
    utf8 = role(sources, COLUMNAR, lambda item: "simdutf8::basic::from_utf8" in item.code, "columnar.utf8_owner")
    require(has_literal_syntax(utf8.raw_body, '#[cfg(feature="simdutf8-validate")]') and has_literal_syntax(utf8.raw_body, '#[cfg(not(feature="simdutf8-validate"))]') and "std::str::from_utf8(bytes).map_err(|_|Error::InvalidUtf8)" in utf8.code and "simdutf8::basic::from_utf8(bytes).map_err(|_|Error::InvalidUtf8)" in utf8.code, "columnar.utf8_deterministic_fallback")
    count = role(sources, COLUMNAR, lambda item: "u32::from_le_bytes(prefix)" in item.code and "enforce_decode_sequence_length" in item.code, "columnar.row_count_owner")
    require("bytes.len().saturating_sub(prefix.len())" in count.code and "Error::LengthMismatch" in count.code and "enforce_decode_sequence_length(u64::from(count))?" in count.code, "columnar.row_count_bounds")


def validate_encoding(sources: dict[str, str]) -> None:
    # Discover the destination-owned skip operation by its role, not its name.
    skip = role(sources, ENCODER, lambda item: "EncoderSink::Counting" in item.code and ".add(" in item.code, "encoding.count_destination_owner")
    require("Ok(true)" in skip.code and "Ok(false)" in skip.code, "encoding.count_destination_isolation")
    constructor = role(sources, ENCODER, lambda item: "Self{sink:EncoderSink::Counting(" in item.code, "encoding.count_constructor_owner")
    counter = operation(sources, CORE, "encoded_payload_len")
    require(f"Encoder::{constructor.name}(" in counter.code and "value.serialize(&mutencoder)?" in counter.code and ".finish()?" in counter.code and "validate_header_flags(flags)?" in counter.code, "encoding.actual_measurement")
    owned = role(sources, CORE, lambda item: f".{skip.name}(" in item.code and "serialize_to_writer_exact(" in item.code, "encoding.measured_emission_owner")
    preceding = owned.source[max(0, owned.start - 20):owned.start]
    require(not re.search(r"\bpub(?:\([^)]*\))?\s*$", preceding), "encoding.measured_emission_private")
    field = operation(sources, CORE, "write_len_prefixed")
    require("encoded_payload_len(value)?" in field.code and f"{owned.name}(value,writer," in field.code, "encoding.field_uses_owned_measurement")
    exact = operation(sources, CORE, "serialize_to_writer_exact")
    binding = re.search(r"letmut(\w+)=ExactLengthWriter::new\(writer,(\w+)\);", exact.code)
    checked = False
    if binding:
        writer_name, expected_name = binding.groups()
        checked = bool(re.search(rf"if{writer_name}\.\w+\(\)\{{returnErr\(Error::LengthMismatch\);\}}", exact.code)) and bool(re.search(rf"{writer_name}\.\w+\(\)=={expected_name}", exact.code))
    require(checked and "result?;" in exact.code and f".{skip.name}(" not in exact.code, "encoding.public_exact_checks_output")
    packed = operation(sources, FIELDS, "write_packed_fields")
    require(".try_reserve_exact(fields.len())" in packed.code and "Error::AllocationFailed" in packed.code and ".checked_mul(" in packed.code, "encoding.packed_allocation_bound")
    require("bits.len()!=fields.len().div_ceil(8)" in packed.code and "byte>>tail!=0" in packed.code, "encoding.packed_bitset_bounds")
    require("encoded_payload_len(*value)?" in packed.code and f"{owned.name}(*value,writer,length)?" in packed.code, "encoding.packed_owned_measurement")
    frame = operation(sources, FRAMES, "write_frame_with_prefix")
    require("encoded_frame_len(value)?" in frame.code and "prefix(writer,frame_len)?" in frame.code and f".{skip.name}(frame_len)?" in frame.code and ".ok_or(Error::NonCanonicalEncoding)" in frame.code, "encoding.frame_owned_measurement")
    emission = role(sources, FRAMES, lambda item: "ExactLengthWriter::new(" in item.code and "FramedPayloadWriter" in item.code, "encoding.frame_emission_owner")
    for error in ("LengthMismatch", "ChecksumMismatch", "NonCanonicalEncoding"):
        require(f"returnErr(Error::{error});" in emission.code, f"encoding.frame_rejects:{error}")
    require("serialize_result?;" in emission.code, "encoding.frame_preserves_child_error")


def validate_codegen(sources: dict[str, str]) -> None:
    binary = reachable(sources, ("derive_norito_serialize",))
    code = "".join(item.code for item in binary)
    require("norito::core::write_len_prefixed(" in code and "norito::core::write_packed_fields(" in code, "codegen.canonical_field_owners")
    generated = [child for item in binary for child in functions(item.raw_body) if child.name == "serialize"]
    require(bool(generated), "codegen.serializer_bodies")
    require(all("EncodeValueDepthGuard::enter()?;" in item.code for item in generated), "codegen.binary_depth_guard")
    require("PackedField::Bytes(" in code and "PackedField::Value(" in code, "codegen.typed_packed_fields")
    require(all("try_reserve_exact(" not in item.code and "Vec::with_capacity(" not in item.code for item in generated), "codegen.runtime_allocation_owner")
    classification = [item for item in binary if has_literal_syntax(item.raw_body, 'path.path.is_ident("u8")') and "syn::Type::Array" in item.code]
    require(bool(classification), "codegen.raw_array_is_u8_only")
    enum_index = role(sources, DERIVE, lambda item: "assigned.insert(index," in item.code, "codegen.enum_index_owner")
    require("rust_discriminant.checked_add(1)" in enum_index.code and "explicit!=codec_index" in enum_index.code, "codegen.explicit_enum_indices")

    json_roots = reachable(sources, ("derive_fast_json", "derive_json_deserialize"))
    parses = [child for item in json_roots for child in functions(item.raw_body) if child.name == "parse"]
    require(bool(parses) and all(item.code.startswith("w.ensure_document_depth()?;") for item in parses), "codegen.json_document_depth")
    strict_blocks = []
    for item in json_roots:
        masked = _mask_non_code(item.raw_body)
        for match in re.finditer(r"if\s+\w+\.deny_unknown_fields\s*\{", masked):
            end = closing_brace(masked, match.end() - 1)
            strict_blocks.append(compact(item.raw_body[match.end():end]))
    require(bool(strict_blocks) and all("norito::json::Error::unknown_field(" in block for block in strict_blocks), "codegen.strict_json_rejection")
    require(any("norito::json::Error::missing_field(" in item.code for item in json_roots), "codegen.structured_missing_fields")
    bounded = reachable(sources, ("derive_fast_json_write",))
    writers = [child for item in bounded for child in functions(item.raw_body) if child.name == "write_json_to"]
    require(bool(writers) and all("JsonWriteSink" in item.signature and "BoundedJsonError" in item.signature for item in writers), "codegen.bounded_json_sink")
    helper = role(sources, JSON_WRITER, lambda item: "JsonWriteSink::unbounded_output(" in item.code, "codegen.json_helper_owner")
    require("BoundedJsonError::Unsupported" in helper.code and "JsonSerialize::json_serialize_to(" in helper.code, "codegen.bounded_json_helper_rejection")


def validate_identity(sources: dict[str, str]) -> None:
    frame_hash = operation(sources, IDENTITY, "frame_hash")
    require("schema_hash_for_name(&T::frame_name())" in frame_hash.code, "identity.frame_hash_uses_declared_name")
    entry = operation(sources, DERIVE, "derive_norito_schema")
    call = re.search(r"schema_identity::(\w+)\(", entry.code)
    require(call is not None, "identity.derive_owner")
    expansion = operation(sources, IDENTITY_DERIVE, call.group(1))
    require("input.generics.params" in expansion.code and "::norito::NoritoSchema>::nominal_name()" in expansion.code, "identity.generic_nominal_arguments")
    require("IntoSchema" not in expansion.code and "NoritoSerialize" not in expansion.code, "identity.no_structural_marker_expansion")


RUNTIME_CONTRACTS = {
    COUNT_TESTS: (
        "nested_counting_visits_each_leaf_once_in_every_layout",
        "counting_never_trusts_public_exact_writer_lengths",
        "nested_buffer_and_checksum_writers_still_receive_real_bytes",
        "count_overflow_is_sticky_even_if_a_serializer_ignores_it",
    ),
    FIELDS: ("packed_fields_reject_invalid_bitsets_before_visiting_or_writing", "packed_fields_reject_changed_lengths_on_real_output"),
    FRAMES: ("nested_prefixed_frames_measure_each_leaf_once_and_preserve_wire_bytes", "prefixed_frame_rejects_length_checksum_and_flag_drift_with_bounded_output"),
    CODEGEN: ("generated_serializers_use_two_argument_field_writers_without_scratch_buffers", "binary_default_attributes_do_not_generate_missing_field_fallbacks", "packed_tuple_descriptors_keep_field_order"),
}


def validate_runtime_registration(sources: dict[str, str]) -> None:
    for path, names in RUNTIME_CONTRACTS.items():
        for name in names:
            matches = [item for item in functions(sources[path]) if item.name == name]
            require(len(matches) == 1, f"runtime.missing:{path}::{name}")
            item = matches[0]
            prefix = sources[path][:item.start]
            attrs = re.search(r"((?:\s*#\[[^\n]*\])+\s*)$", prefix)
            require(attrs is not None and "#[test]" in attrs.group(1) and "ignore" not in attrs.group(1), f"runtime.disabled:{path}::{name}")
            require("assert" in item.code, f"runtime.no_assertion:{path}::{name}")


VALIDATORS = (validate_columnar, validate_encoding, validate_codegen, validate_identity, validate_runtime_registration)


def read_sources(root: Path) -> dict[str, str]:
    sources = {}
    for relative in SOURCE_FILES:
        path = root / relative
        require(path.is_file() and not path.is_symlink(), f"owner.file:{relative}")
        require(path.resolve().is_relative_to(root.resolve()), f"owner.escape:{relative}")
        sources[relative] = path.read_text(encoding="utf-8")
    return sources


def validate(sources: dict[str, str]) -> None:
    for path in SOURCE_FILES:
        require(path in sources, f"owner.file:{path}")
    for validator in VALIDATORS:
        validator(sources)


def verify_history(record: dict, read_blob) -> list[dict]:
    """Authenticate supplied historical IDs only; never invent missing postimages."""
    require(isinstance(record.get("images"), list) and bool(record["images"]), "history.images_missing")
    rows = []
    for image in record["images"]:
        blob = image.get("git_blob")
        row = {"owner": image["owner"], "role": image["role"], "git_blob": blob}
        payload = read_blob(blob) if blob is not None else None
        if payload is None:
            row["status"] = "unverified"
        elif hashlib.sha256(payload).hexdigest() != image["sha256"] or (image.get("recorded_lines") is not None and len(payload.splitlines()) != image["recorded_lines"]):
            row["status"] = "mismatch"
        else:
            row["status"] = "verified"
        rows.append(row)
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--history", type=Path, help="Verify historical evidence separately; missing images exit 3.")
    args = parser.parse_args()
    if args.history:
        def read_blob(blob: str) -> bytes | None:
            result = subprocess.run(["git", "cat-file", "blob", blob], cwd=args.root, capture_output=True)
            return result.stdout if result.returncode == 0 else None
        rows = verify_history(json.loads(args.history.read_text()), read_blob)
        print(json.dumps({"historical_images": rows}, indent=2))
        return 1 if any(row["status"] == "mismatch" for row in rows) else (3 if any(row["status"] == "unverified" for row in rows) else 0)
    try:
        validate(read_sources(args.root))
    except (ContractError, OSError) as error:
        print(f"Norito codec source contract failed: {error}")
        return 1
    print("Current Norito source ownership contracts pass; Rust behavior, size and provenance require separate qualification.")
    return 0



# Explicit on-wire descriptor and adaptive tag values, independent of helper layout.
COLUMNAR_WIRE_VALUES = {
    'DESC_U64_STR_BOOL': 19,
    'DESC_U64_DICT_STR_BOOL': 147,
    'DESC_U64_DELTA_STR_BOOL': 83,
    'DESC_U64_OPTSTR_BOOL': 27,
    'DESC_U64_DELTA_OPTSTR_BOOL': 91,
    'DESC_U64_OPTU32_BOOL': 28,
    'DESC_U64_DELTA_OPTU32_BOOL': 92,
    'DESC_U64_ENUM_BOOL': 97,
    'DESC_U64_DELTA_ENUM_BOOL': 99,
    'DESC_U64_ENUM_BOOL_CODEDELTA': 101,
    'DESC_U64_DELTA_ENUM_BOOL_CODEDELTA': 103,
    'DESC_U64_ENUM_BOOL_DICT': 225,
    'DESC_U64_DELTA_ENUM_BOOL_DICT': 227,
    'DESC_U64_ENUM_BOOL_DICT_CODEDELTA': 229,
    'DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA': 231,
    'DESC_U64_BYTES_BOOL': 20,
    'DESC_U64_DELTA_BYTES_BOOL': 84,
    'DESC_U64_U32_BOOL': 33,
    'DESC_U64_DELTA_U32_BOOL': 35,
    'DESC_U64_U32DELTA_BOOL': 37,
    'DESC_U64_DELTA_U32DELTA_BOOL': 39,
    'DESC_U64_STR_U32_BOOL': 51,
    'DESC_U64_DELTA_STR_U32_BOOL': 115,
    'DESC_U64_STR_U32DELTA_BOOL': 55,
    'DESC_U64_DELTA_STR_U32DELTA_BOOL': 119,
    'DESC_U64_DICT_STR_U32_BOOL': 179,
    'DESC_U64_DELTA_DICT_STR_U32_BOOL': 243,
    'DESC_U64_DICT_STR_U32DELTA_BOOL': 183,
    'DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL': 247,
    'DESC_U64_BYTES_U32_BOOL': 52,
    'DESC_U64_DELTA_BYTES_U32_BOOL': 116,
    'DESC_U64_BYTES_U32DELTA_BOOL': 56,
    'DESC_U64_DELTA_BYTES_U32DELTA_BOOL': 120,
    'ADAPTIVE_TAG_AOS': 0,
    'ADAPTIVE_TAG_NCB': 1,
    'ADAPTIVE_ENUM_TAG_AOS': 0,
    'ADAPTIVE_ENUM_TAG_NCB': 1,
}


if __name__ == "__main__":
    raise SystemExit(main())
