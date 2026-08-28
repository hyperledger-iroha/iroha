#!/usr/bin/env python3
"""Fail closed on the landed Kotodama IR helper compaction.

The guard authenticates the indexed opening through Git and the exact landed
postimage, then independently seals the public, test, fixture, constructor,
fallback, traversal, and helper-call contracts behind the compaction.  Normal
execution derives the repository root from this file and reads the compiled
``crates/kotodama_lang/src/ir.rs``.  Explicit test-only environment overrides
allow this same module to validate an isolated candidate without weakening the
landed path contract.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import unittest
from pathlib import Path


TEST_OVERRIDE_GATE = "KOTODAMA_IR_GUARD_TEST_OVERRIDE"
TEST_ROOT_OVERRIDE = "KOTODAMA_IR_GUARD_TEST_ROOT"
TEST_SOURCE_OVERRIDE = "KOTODAMA_IR_GUARD_TEST_SOURCE"

_test_override = os.environ.get(TEST_OVERRIDE_GATE) == "1"
_root_override = os.environ.get(TEST_ROOT_OVERRIDE)
_source_override = os.environ.get(TEST_SOURCE_OVERRIDE)
if _test_override:
    if not _root_override or not _source_override:
        raise RuntimeError("test override requires explicit root and source paths")
    REPO_ROOT = Path(_root_override).resolve(strict=True)
    SOURCE_PATH = Path(_source_override).resolve(strict=True)
else:
    if _root_override is not None or _source_override is not None:
        raise RuntimeError("test paths require KOTODAMA_IR_GUARD_TEST_OVERRIDE=1")
    REPO_ROOT = Path(__file__).resolve().parents[2]
    SOURCE_PATH = REPO_ROOT / "crates/kotodama_lang/src/ir.rs"

PUBLIC_LEAF = REPO_ROOT / "crates/kotodama_lang/src/ir/tests/public_argument_record_abi.rs"
TAIL_LEAF = REPO_ROOT / "crates/kotodama_lang/src/ir_tail_tests.rs"
FIXTURE_MANIFEST = REPO_ROOT / "crates/kotodama_lang/kotodama_fixtures_v1.manifest.json"

OPENING_LOC = 11_725
OPENING_BYTES = 432_298
OPENING_SHA256 = "cfcc8798b026b9c5a697a2c895a4df0aea6a08c565bcba5332c9733a00b3f377"
OPENING_GIT_BLOB = "ec7f6f8a9565b6cee290769439f1c7b7fb267224"
POST_LOC = 10_707
POST_BYTES = 403_581
POST_SHA256 = "17ca6648456c079cbaaa80fdc668fbce70a419e14c766ecb8567277109b4400a"
POST_GIT_BLOB = "29dbc7b301b314a1409b578bf7f1cd4e7c16532a"
MINIMUM_RUST_LINE_REDUCTION = 1_000
PRODUCTION_LOC = 7_580
TEST_SUFFIX_LOC = 3_127
TEST_SUFFIX_SHA256 = "e32e6b27f19893b1a083df040bacf311041d340527bf5e828b97728b08e1397c"

PUBLIC_API_COUNT = 20
PUBLIC_API_SHA256 = "7680ab2ee86f8127d2f8b3184b4672d25497b82e1a21512438ade78d8f9cc2d1"
INSTR_VARIANT_COUNT = 193
INSTR_VARIANTS_SHA256 = "23584254a4bbda2d3a14c5ad5b55ab8168f70c9be9e4c4ff35c6593adcfc3b9c"
MAIN_TEST_COUNT = 96
MAIN_TESTS_NEWLINE_SHA256 = "527e21bb794c3cd98755bfeafc9eb864bd850600f757c628b02d89cdfa52dc06"
ALL_TEST_COUNT = 100
ALL_TESTS_JSON_SHA256 = "c4bae4325e0ff9eba1cac7c80ec35cbea1994e78749d9c7f9f6a88e5de07a3db"
PUBLIC_LEAF_SHA256 = "9e7a57eb9ac866072caee317f2b5af503c300de58729e184305ba000e511c16b"
TAIL_LEAF_SHA256 = "57a81a87cdf5a70ea8a8235c369cb42689a508c428f182d16e0f75b909e3b801"
ASSET_COUNT = 54
ASSET_LEDGER_SHA256 = "f453b53114ea7df3e05cebc3e0f089664ab3e603e6debabe686b94e2073fb2bc"
PRODUCTION_LITERAL_COUNT = 164
PRODUCTION_LITERALS_SHA256 = "4186c4ae58efcf2aa63ed8e9ddef800ab46453e670f19bce53647c8b36af55d3"

EXPECTED_PUBLIC_API = (
    "TEST_TRIGGER_EVENT_OVERRIDE_KEY",
    "Temp",
    "Label",
    "Program",
    "Function",
    "BasicBlock",
    "WideNumericKind",
    "NumericRoundOp",
    "DecimalToIntOp",
    "Instr",
    "VendorInstructionKind",
    "DataRefKind",
    "Terminator",
    "lower",
    "lower_with_cap",
    "lower_with_cap_and_test_mode",
    "LoweringFailure",
    "lower_with_cap_and_test_mode_diagnostics",
    "entrypoint_return_schema",
    "entrypoint_argument_schema",
)

# name, formatted definition line count, definition SHA-256, non-definition calls
HELPER_LEDGER = (
    ("emit_data_ref_into", 3, "2092dd17d9648570f62db85e206214f12d26dde36b909a9a36aecef8e60ca511", 6),
    ("emit_data_ref", 5, "f6d2d2b879ad391ab1d3117e710f6764880b74d03a50917e83a502f079fd4d7b", 7),
    ("emit_copy", 3, "a5109c1511321132db077541cee39330079718fe053cab00c4b1a7ad8abc8db4", 29),
    ("emit_binary", 10, "04a2e9ab78acf52c1644010273eb4b56bff00bc6b74ad6abdf5d1afea44ca55c", 39),
    ("emit_numeric_compare", 17, "3869280b42593923ed5ec12f07d053626451537cd09446c957f8c699bc79a5f6", 2),
    ("emit_pointer_eq", 5, "9f5cb3da32cf76b5efcb8c5939063fc4f1eb47bb047b9fc360e0d00cec0244ae", 1),
    ("emit_unary", 5, "3be46bec54bbdc6efe920bc7cc1eb785064f9520e541da8e791c81c455b76b9d", 4),
    ("emit_load64_imm", 5, "1da7afdd041c79382c39638966be626cea82bd9f6a3f2aa5691ff0d8a70156d0", 17),
    ("emit_store64_imm", 3, "a24a5988a8ca824547c1a56a87a12b0a01747334a249178178833312393c9c59", 9),
    ("emit_tuple_get", 5, "cd4b3deb211b44b4e7d8936f0eaea130935fb74583a32f1c64f86fa1eb4cb9a5", 12),
    ("emit_tuple_pack", 5, "9916b7dd39fbe5825b00f8f469a1bf9b97bcdadbed92e3f91e9aa898553dfd0e", 9),
    ("emit_state_get", 5, "707bced04359183231a86d2a7d399a1d1377bc9e0121679ed378c09d76a2e193", 5),
    ("emit_alloc", 5, "4680cf37d368d00ac0c2e4b9da8d94850913b9294242ca4795eb5620c5d2edf2", 4),
    ("emit_pointer_to_norito", 5, "bd9859bfd0c0be7da4b6caf2a3548f745f334720a9dc86b9c9f2ce2f1038d651", 3),
    ("emit_pointer_from_norito", 5, "b0966c2c33659d625bf45357bba146df0f6e07848c62b486ff529e3571c823d9", 4),
    ("append_value_word_types", 21, "b453c793ec4a43c19ef335fc30e832c9e689f3d39a539c46c4091fa695f6b4e9", 4),
    ("lower_map_fallback", 105, "3a50ad3b661e6a4b8ec0540ee9ec7d75c26f21dffa77f1721daab450fbf407c9", 3),
    ("lower_take2_pair", 19, "2486003cdf0185ac7ddc57ad41c6a372867748b05750dc4bb885c718412bf5ab", 2),
    ("seal_unreachable_continuation", 5, "2d13cb5e6ebdf434c6037a926a427cd4eb0c7531eaf01d8aa9bb24379527ccff", 2),
)
HELPER_LEDGER_SHA256 = "68f03c3bca2d96edd107303f86968c6efa61dff16c92b2e467fb4e60a1219916"
HELPER_CALL_LEDGER_COUNT = 162
HELPER_CALL_LEDGER_SHA256 = "80bd617bf02f1c461adb4d3488dc44f2a9d45aac7681bc9cc7242f86ade6a47a"

EXPECTED_MAP_FALLBACK_ROUTES = (
    ("GetOrDefault", "ctx,&args[0],&args[1],&args[2],MapFallback::Eager,vars"),
    ("GetOr", "ctx,&args[0],&args[1],&args[2],MapFallback::Lazy,vars"),
    ("Ensure", "ctx,&args[0],&args[1],&args[2],MapFallback::Insert,vars"),
)
TAKE2_ROUTING_SHA256 = "2d5f3d141b1f8fd640547ae19dba829cc68b8fd972c90d940831a0c2d742fa14"

RETAINED_SORACLOUD_REQUEST_BUILTINS = (
    "SoracloudReadConfig",
    "SoracloudReadSecretEnvelope",
)
RETIRED_SORACLOUD_REQUEST_BUILTINS = (
    "SoracloudReadSecret",
    "SoracloudReadCredential",
    "SoracloudEgressFetch",
)

DIRECT_INSTR_COUNTS = {
    "DataRef": 2,
    "Copy": 4,
    "Binary": 11,
    "NumericCompare": 6,
    "PointerEq": 4,
    "Unary": 1,
    "Load64Imm": 4,
    "Store64Imm": 2,
    "TupleGet": 1,
    "TuplePack": 1,
    "StateGet": 5,
    "Alloc": 1,
    "PointerToNorito": 1,
    "PointerFromNorito": 1,
}

PUBLIC_RE = re.compile(
    r"^pub(?:\(crate\))?\s+(?:const|struct|enum|type|fn)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)",
    re.MULTILINE,
)
DIRECT_TEST_RE = re.compile(
    r"^\s*#\[test\]\s*\n"
    r"(?:\s*#\[[^\n]+\]\s*\n)*"
    r"\s*(?:pub(?:\([^\n)]*\))?\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
    re.MULTILINE,
)
ALIAS_TEST_RE = re.compile(
    r"^\s*alias_lowering_case!\(\s*\n?\s*([a-z0-9_]+)\s*,",
    re.MULTILINE,
)
INCLUDE_STR_RE = re.compile(r'include_str!\(\s*"([^"]+)"\s*\)')
RUST_LITERAL_RE = re.compile(
    r'(?<![A-Za-z0-9_])(?:b)?"(?:\\.|[^"\\])*"'
    r'|(?<![A-Za-z0-9_])r(?P<hashes>#{0,16})".*?"(?P=hashes)',
    re.DOTALL,
)
RAW_STRING_START = re.compile(r'(?:b?r)(#*)"')
TEST_ANCHOR = "#[cfg(test)]\n"


class GuardError(AssertionError):
    """Raised when a protected source contract changes."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode("utf-8")
    return hashlib.sha256(data).hexdigest()


def _json_digest(value: object) -> str:
    return _sha256(json.dumps(value, separators=(",", ":")).encode("utf-8"))


def _git_blob(data: bytes) -> str:
    header = b"blob " + str(len(data)).encode("ascii") + b"\0"
    return hashlib.sha1(header + data).hexdigest()


def _regular_bytes(path: Path, root: Path | None = None) -> bytes:
    _require(not path.is_symlink(), f"symlink is not allowed: {path}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise GuardError(f"cannot stat {path}: {error}") from error
    _require(stat.S_ISREG(mode), f"not a regular file: {path}")
    resolved = path.resolve(strict=True)
    if root is not None:
        try:
            resolved.relative_to(root.resolve(strict=True))
        except ValueError as error:
            raise GuardError(f"path escapes repository: {path}") from error
    return path.read_bytes()


def _opening_blob() -> bytes:
    try:
        return subprocess.check_output(
            ["git", "cat-file", "blob", OPENING_GIT_BLOB],
            cwd=REPO_ROOT,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GuardError(f"authenticated opening blob is unavailable: {error}") from error


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
            _require(depth == 0, "unterminated Rust block comment")
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
            _require(
                bool(stack) and pairs[stack[-1]] == character,
                f"unbalanced Rust delimiter at byte {cursor}",
            )
            stack.pop()
            if not stack:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust delimiter")


def _item_region_at(source: str, start: int, label: str) -> str:
    try:
        brace = source.index("{", start)
    except ValueError as error:
        raise GuardError(f"missing opening brace: {label}") from error
    return source[start : _matching_delimiter(source, brace) + 1]


def _item_region(source: str, marker: str) -> str:
    try:
        start = source.index(marker)
    except ValueError as error:
        raise GuardError(f"missing item marker: {marker}") from error
    return _item_region_at(source, start, marker)


def _function_region(source: str, name: str) -> str:
    matches = list(re.finditer(rf"(?m)^fn {re.escape(name)}\s*\(", source))
    _require(len(matches) == 1, f"{name} definition count changed")
    return _item_region_at(source, matches[0].start(), name)


def _function_spans(source: str) -> list[tuple[int, int, str]]:
    pattern = re.compile(
        r"(?m)^[ \t]*(?:(?:pub(?:\([^\n)]*\))?)\s+)?"
        r"(?:(?:async|const|unsafe)\s+)*fn\s+([A-Za-z_]\w*)"
        r"(?:<[^\{\n]*>)?\s*\("
    )
    spans: list[tuple[int, int, str]] = []
    for match in pattern.finditer(source):
        opening_paren = source.find("(", match.start(), match.end())
        closing_paren = _matching_delimiter(source, opening_paren)
        opening_brace = source.find("{", closing_paren)
        _require(opening_brace >= 0, f"missing function body: {match.group(1)}")
        spans.append(
            (match.start(), _matching_delimiter(source, opening_brace), match.group(1))
        )
    return spans


def _helper_call_ledger(production: str) -> list[list[str]]:
    names = tuple(entry[0] for entry in HELPER_LEDGER)
    pattern = re.compile(r"\b(" + "|".join(map(re.escape, names)) + r")\s*\(")
    spans = _function_spans(production)
    ledger: list[list[str]] = []
    for match in pattern.finditer(production):
        before = production[max(0, match.start() - 20) : match.start()]
        if re.search(r"\bfn\s*$", before):
            continue
        opening = production.find("(", match.start(), match.end())
        closing = _matching_delimiter(production, opening)
        containers = [span for span in spans if span[0] <= match.start() <= span[1]]
        _require(bool(containers), f"helper call is outside a function: {match.group(1)}")
        caller = max(containers, key=lambda span: span[0])[2]
        call = re.sub(r"\s+", "", production[match.start() : closing + 1])
        ledger.append([caller, match.group(1), call])
    return ledger


def _fallback_routes(production: str) -> tuple[tuple[str, str], ...]:
    surface = _function_region(production, "lower_surface_builtin_call")
    pattern = re.compile(
        r"Builtin::(GetOrDefault|GetOr|Ensure)\s*=>\s*(?:\{\s*)?"
        r"lower_map_fallback\s*\("
    )
    routes: list[tuple[str, str]] = []
    for match in pattern.finditer(surface):
        opening = surface.find("(", match.start(), match.end())
        closing = _matching_delimiter(surface, opening)
        arguments = re.sub(r"\s+", "", surface[opening + 1 : closing])
        routes.append((match.group(1), arguments))
    return tuple(routes)


def _take2_routing_region(production: str) -> str:
    surface = _function_region(production, "lower_surface_builtin_call")
    start_marker = "        Builtin::KeysTake2 | Builtin::ValuesTake2 => {"
    end_marker = "        Builtin::TestInvokeEntrypoint"
    _require(surface.count(start_marker) == 1, "take2 combined arm changed")
    start = surface.index(start_marker)
    _require(surface.count(end_marker, start) == 1, "take2 routing terminator changed")
    return surface[start : surface.index(end_marker, start)]


def _split_candidate(source: str) -> tuple[str, str]:
    position = source.rfind(TEST_ANCHOR)
    _require(position >= 0, "final cfg(test) anchor is missing")
    production, suffix = source[:position], source[position:]
    _require(len(production.splitlines()) == PRODUCTION_LOC, "production LOC changed")
    _require(len(suffix.splitlines()) == TEST_SUFFIX_LOC, "test-suffix LOC changed")
    _require(
        suffix.count('if *kind == DataRefKind::Json && value == "{}" {') == 1
        and 'value == "{}\\n"' not in suffix,
        "canonical JSON fixture spelling changed",
    )
    _require(_sha256(suffix) == TEST_SUFFIX_SHA256, "test suffix changed")
    return production, suffix


def _enum_variants(source: str) -> tuple[str, ...]:
    region = _item_region(source, "pub enum Instr")
    return tuple(
        re.findall(r"^    ([A-Z][A-Za-z0-9_]*)\s*(?:\{|\(|,)", region, re.MULTILINE)
    )


def _leaf_test_names(source: str) -> tuple[str, ...]:
    return tuple(DIRECT_TEST_RE.findall(source))


def _test_inventory(
    source: str, public_leaf: str, tail_leaf: str
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    _, suffix = _split_candidate(source)
    events: list[tuple[int, tuple[str, ...], str]] = []
    for match in DIRECT_TEST_RE.finditer(suffix):
        events.append((match.start(), (match.group(1),), "direct"))
    for match in ALIAS_TEST_RE.finditer(suffix):
        events.append((match.start(), (match.group(1),), "alias"))

    public_names = _leaf_test_names(public_leaf)
    tail_names = _leaf_test_names(tail_leaf)
    _require(len(public_names) == 2, "public test-leaf inventory changed")
    _require(len(tail_names) == 2, "tail test-leaf inventory changed")
    for marker, names in (
        ('include!("ir/tests/public_argument_record_abi.rs")', public_names),
        ('include!("ir_tail_tests.rs")', tail_names),
    ):
        _require(suffix.count(marker) == 1, f"include contract changed: {marker}")
        events.append((suffix.index(marker), names, marker))

    all_names: list[str] = []
    main_names: list[str] = []
    for _, names, kind in sorted(events):
        all_names.extend(names)
        if kind in ("direct", "alias"):
            main_names.extend(names)
    return tuple(main_names), tuple(all_names)


def _manifest_ir_names() -> tuple[str, ...]:
    try:
        payload = json.loads(_regular_bytes(FIXTURE_MANIFEST, REPO_ROOT))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise GuardError(f"invalid fixture manifest: {error}") from error
    entries = [
        entry
        for entry in payload.get("source_files", [])
        if entry.get("path") == "crates/kotodama_lang/src/ir.rs"
    ]
    _require(len(entries) == 1, "IR fixture-manifest entry changed")
    entry = entries[0]
    names = tuple(entry.get("test_names", ()))
    _require(
        entry.get("test_names_sha256") == MAIN_TESTS_NEWLINE_SHA256,
        "manifest IR test digest changed",
    )
    _require(
        _sha256(("\n".join(names) + "\n").encode("utf-8"))
        == MAIN_TESTS_NEWLINE_SHA256,
        "manifest IR test names do not match their digest",
    )
    return names


def _asset_ledger(
    source: str,
    tail_leaf: str,
    overrides: dict[str, bytes] | None = None,
) -> list[list[object]]:
    overrides = overrides or {}
    owners = (
        ("crates/kotodama_lang/src/ir.rs", source),
        ("crates/kotodama_lang/src/ir_tail_tests.rs", tail_leaf),
    )
    ledger: list[list[object]] = []
    for owner, text in owners:
        owner_dir = (REPO_ROOT / owner).parent
        for include in INCLUDE_STR_RE.findall(text):
            unresolved = owner_dir / include
            on_disk = _regular_bytes(unresolved, REPO_ROOT)
            path = unresolved.resolve(strict=True)
            try:
                relative = path.relative_to(REPO_ROOT.resolve(strict=True)).as_posix()
            except ValueError as error:
                raise GuardError(f"asset include escapes repository: {include}") from error
            data = overrides.get(relative, on_disk)
            ledger.append([relative, len(data), len(data.splitlines()), _sha256(data)])
    _require(len(ledger) == ASSET_COUNT, "included asset count changed")
    _require(len({entry[0] for entry in ledger}) == ASSET_COUNT, "duplicate asset include")
    _require(
        {Path(str(entry[0])).parent.as_posix() for entry in ledger}
        == {
            "crates/kotodama_lang/src/ir/fixtures/v1",
            "crates/kotodama_lang/src/ir/test_sources",
        },
        "asset directory surface changed",
    )
    _require(_json_digest(ledger) == ASSET_LEDGER_SHA256, "asset ledger changed")
    return ledger


def _validate_opening(data: bytes) -> None:
    _require(len(data.splitlines()) == OPENING_LOC, "opening LOC changed")
    _require(len(data) == OPENING_BYTES, "opening byte count changed")
    _require(_sha256(data) == OPENING_SHA256, "opening SHA-256 changed")
    _require(_git_blob(data) == OPENING_GIT_BLOB, "opening Git blob changed")
    _require(
        OPENING_LOC - POST_LOC >= MINIMUM_RUST_LINE_REDUCTION,
        "recorded Rust-line reduction no longer clears the gate",
    )


def _validate_forbidden_seams(production: str) -> None:
    for token in (
        "macro_rules!",
        "$body",
        "$action",
        "$step",
        "$assertion",
        "rustfmt::skip",
        "include!",
        "include_str!",
        "include_bytes!",
        "#[path",
        "std::fs",
        "fs::read",
        "read_to_string",
    ):
        _require(token not in production, f"forbidden production token: {token}")
    for pattern in (
        r"\b(?:dyn|impl)\s+Fn(?:Mut|Once)?\b",
        r"(?:^|[=:,<(])\s*fn\s*\(",
        r"\b(?:struct|enum|type)\s+(?:Action|Step|Body|Assertion)\b",
        r"(?m)^\s*macro\s+[A-Za-z_]",
        r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_]\w*\s*;",
    ):
        _require(re.search(pattern, production, re.MULTILINE) is None, f"forbidden seam: {pattern}")


def _validate_candidate(
    source: str,
    public_leaf: str,
    tail_leaf: str,
    *,
    check_full_hash: bool = True,
    asset_overrides: dict[str, bytes] | None = None,
) -> None:
    data = source.encode("utf-8")
    _require(len(data.splitlines()) == POST_LOC, "postimage LOC changed")
    if check_full_hash:
        _require(len(data) == POST_BYTES, "postimage byte count changed")
        _require(_sha256(data) == POST_SHA256, "postimage SHA-256 changed")
        _require(_git_blob(data) == POST_GIT_BLOB, "postimage Git blob changed")
    production, _ = _split_candidate(source)
    _validate_forbidden_seams(production)

    public_api = tuple(PUBLIC_RE.findall(production))
    _require(len(public_api) == PUBLIC_API_COUNT, "public API count changed")
    _require(public_api == EXPECTED_PUBLIC_API, "public API name/order changed")
    _require(_json_digest(public_api) == PUBLIC_API_SHA256, "public API digest changed")

    variants = _enum_variants(production)
    _require(len(variants) == INSTR_VARIANT_COUNT, "Instr variant count changed")
    _require(len(set(variants)) == INSTR_VARIANT_COUNT, "duplicate Instr variant")
    _require(_json_digest(variants) == INSTR_VARIANTS_SHA256, "Instr variants changed")

    literals = tuple(sorted({match.group(0) for match in RUST_LITERAL_RE.finditer(production)}))
    _require(len(literals) == PRODUCTION_LITERAL_COUNT, "production literal surface changed")
    _require(_json_digest(literals) == PRODUCTION_LITERALS_SHA256, "diagnostic/literal text changed")
    for name in RETIRED_SORACLOUD_REQUEST_BUILTINS:
        _require(
            re.search(rf"\bBuiltin::{re.escape(name)}\b", production) is None,
            f"retired Soracloud request lowering returned: {name}",
        )
    for name in RETAINED_SORACLOUD_REQUEST_BUILTINS:
        _require(
            len(re.findall(rf"\bBuiltin::{re.escape(name)}\b", production)) == 1,
            f"retained Soracloud request lowering changed: {name}",
        )

    _require(_sha256(public_leaf) == PUBLIC_LEAF_SHA256, "public test leaf changed")
    _require(_sha256(tail_leaf) == TAIL_LEAF_SHA256, "tail test leaf changed")
    main_tests, all_tests = _test_inventory(source, public_leaf, tail_leaf)
    _require(len(main_tests) == MAIN_TEST_COUNT, "main test count changed")
    _require(all_tests == _manifest_ir_names(), "compiled test IDs/order changed")
    _require(len(all_tests) == ALL_TEST_COUNT, "compiled test count changed")
    _require(len(set(all_tests)) == ALL_TEST_COUNT, "duplicate compiled test ID")
    _require(_json_digest(all_tests) == ALL_TESTS_JSON_SHA256, "100 test IDs/order changed")
    _asset_ledger(source, tail_leaf, asset_overrides)

    observed_helpers: list[list[object]] = []
    for name, lines, digest, calls in HELPER_LEDGER:
        region = _function_region(production, name)
        observed_calls = len(re.findall(rf"\b{re.escape(name)}\s*\(", production)) - 1
        observed = [name, len(region.splitlines()), _sha256(region), observed_calls]
        observed_helpers.append(observed)
        _require(observed == [name, lines, digest, calls], f"{name} contract changed")
        _require(
            f"#[inline]\nfn {name}" not in production,
            f"{name} must remain free of forced inline expansion",
        )
    _require(_json_digest(observed_helpers) == HELPER_LEDGER_SHA256, "helper ledger changed")

    helper_calls = _helper_call_ledger(production)
    _require(len(helper_calls) == HELPER_CALL_LEDGER_COUNT, "helper call count changed")
    _require(
        _json_digest(helper_calls) == HELPER_CALL_LEDGER_SHA256,
        "helper call ledger changed",
    )
    _require(
        _fallback_routes(production) == EXPECTED_MAP_FALLBACK_ROUTES,
        "Builtin-to-MapFallback routing changed",
    )
    _require(
        _sha256(_take2_routing_region(production)) == TAKE2_ROUTING_SHA256,
        "take2 Builtin routing changed",
    )

    for variant, expected in DIRECT_INSTR_COUNTS.items():
        actual = len(re.findall(rf"Instr::{variant}\s*\{{", production))
        _require(actual == expected, f"direct Instr::{variant} construction count changed")
    _require(production.count("current_instr(Instr::") == 261, "instruction construction proxy changed")
    _require(production.count(".new_temp()") == 188, "temporary construction proxy changed")

    fallback = _item_region(production, "enum MapFallback")
    _require(
        _sha256(fallback) == "902f09143ea656a62778693e53165df3988a338f07242aa180c7e997b4acfc3c",
        "fallback variants changed",
    )
    _require(
        "#[derive(Clone, Copy, PartialEq, Eq)]\nenum MapFallback" in production,
        "fallback must remain a fixed typed enum",
    )
    _require(
        production.count("fallback == MapFallback::Insert") == 2,
        "insert fallback tests changed",
    )
    _require("matches!(fallback" not in production, "fallback must remain macro-free")
    for mode, count in (("Eager", 2), ("Lazy", 2), ("Insert", 4)):
        _require(production.count(f"MapFallback::{mode}") == count, f"{mode} fallback routing changed")

    _require(production.count("fn append_value_word_types(") == 1, "shared traversal definition changed")
    _require("fn append(" not in production, "duplicated nested value-word traversal returned")
    _require(
        "discard_empty_unreferenced_continuation" not in production,
        "stale donor continuation behavior returned",
    )
    _require(
        production.count("seal_unreachable_continuation(") == 3,
        "current continuation sealing changed",
    )
    _require(
        production.count("crate::session::run_with_compiler_stack(move || {") == 1,
        "public lowering no longer uses the bounded compiler worker exactly once",
    )
    _require(
        "compiler could not allocate the bounded stack required to lower source nesting"
        in production,
        "bounded lowering worker failure diagnostic changed",
    )


def _repository_contract() -> None:
    opening = _opening_blob()
    source_root = None if _test_override else REPO_ROOT
    candidate = _regular_bytes(SOURCE_PATH, source_root)
    public_leaf = _regular_bytes(PUBLIC_LEAF, REPO_ROOT).decode("utf-8")
    tail_leaf = _regular_bytes(TAIL_LEAF, REPO_ROOT).decode("utf-8")
    _validate_opening(opening)
    _validate_candidate(candidate.decode("utf-8"), public_leaf, tail_leaf)


class KotodamaIrCompactionSourceTest(unittest.TestCase):
    """Authenticate the indexed opening and exact landed source contract."""

    def test_repository_contract(self) -> None:
        _repository_contract()


class KotodamaIrCompactionMutationTest(unittest.TestCase):
    """Prove representative one-axis mutations fail structural checks."""

    @classmethod
    def setUpClass(cls) -> None:
        source_root = None if _test_override else REPO_ROOT
        cls.source = _regular_bytes(SOURCE_PATH, source_root).decode("utf-8")
        cls.public_leaf = _regular_bytes(PUBLIC_LEAF, REPO_ROOT).decode("utf-8")
        cls.tail_leaf = _regular_bytes(TAIL_LEAF, REPO_ROOT).decode("utf-8")

    def assert_source_mutation_fails(self, mutated: str, pattern: str) -> None:
        self.assertNotEqual(mutated, self.source)
        with self.assertRaisesRegex(GuardError, pattern):
            _validate_candidate(
                mutated,
                self.public_leaf,
                self.tail_leaf,
                check_full_hash=False,
            )

    def test_api_variant_and_test_id_mutations_fail(self) -> None:
        self.assert_source_mutation_fails(
            self.source.replace("pub struct Temp", "pub struct Tamp", 1),
            "public API",
        )
        self.assert_source_mutation_fails(
            self.source.replace("    Const {", "    Konst {", 1),
            "Instr variants",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "fn malformed_typed_member_access_fails_closed_during_lowering",
                "fn malformed_typed_member_access_fails_shut_during_lowering",
                1,
            ),
            "test suffix",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                '"internal error: missing lowered parameter `{}`"',
                '"internal error: absent lowered parameter `{}`"',
                1,
            ),
            "diagnostic/literal text",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "Builtin::SoracloudReadConfig",
                "Builtin::SoracloudReadSecret",
                1,
            ),
            "retired Soracloud request lowering returned",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                'if *kind == DataRefKind::Json && value == "{}" {',
                'if *kind == DataRefKind::Json && value == "{}\\n" {',
                1,
            ),
            "canonical JSON fixture spelling changed",
        )

    def test_constructor_fallback_and_traversal_mutations_fail(self) -> None:
        self.assert_source_mutation_fails(
            self.source.replace(
                "let table = emit_alloc(ctx, bytes);",
                "let table = emit_state_get(ctx, bytes);",
                1,
            ),
            "emit_(?:alloc|state_get) contract",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "MapFallback::Eager, vars)",
                "MapFallback::Insert, vars)",
                1,
            ),
            "helper call ledger",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "fn runtime_word_is_pointer(ty: &Type) -> bool {",
                "fn append(ty: &Type, words: &mut Vec<Type>) {",
                1,
            ),
            "duplicated nested value-word traversal",
        )

    def test_exact_helper_call_mutations_fail(self) -> None:
        eager_anchor = "MapFallback::Eager, vars)"
        lazy_anchor = "MapFallback::Lazy, vars)"
        temporary_anchor = "MapFallback::__SWAP__, vars)"
        swapped_modes = self.source.replace(eager_anchor, temporary_anchor, 1)
        swapped_modes = swapped_modes.replace(lazy_anchor, "MapFallback::Eager, vars)", 1)
        swapped_modes = swapped_modes.replace(temporary_anchor, "MapFallback::Lazy, vars)", 1)
        self.assert_source_mutation_fails(swapped_modes, "helper call ledger")

        map_key_anchor = (
            "lower_map_fallback(ctx, &args[0], &args[1], &args[2], "
            "MapFallback::Eager, vars)"
        )
        map_key_replacement = (
            "lower_map_fallback(ctx, &args[1], &args[0], &args[2], "
            "MapFallback::Eager, vars)"
        )
        self.assert_source_mutation_fails(
            self.source.replace(map_key_anchor, map_key_replacement, 1),
            "helper call ledger",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "let keep_going = emit_binary(ctx, BinaryOp::Lt, index, source_len);",
                "let keep_going = emit_binary(ctx, BinaryOp::Lt, source_len, index);",
                1,
            ),
            "helper call ledger",
        )
        self.assert_source_mutation_fails(
            self.source.replace(
                "let (key, value) = lower_take2_pair(ctx, args, vars);",
                "let (key, value) = lower_take2_pair(ctx, &args[1..], vars);",
                1,
            ),
            "helper call ledger",
        )

    def test_forbidden_callback_macro_and_relocation_mutations_fail(self) -> None:
        anchor = "//! Intermediate representation for Kotodama programs."
        self.assert_source_mutation_fails(
            self.source.replace(anchor, "// impl Fn callback seam", 1),
            "forbidden seam",
        )
        self.assert_source_mutation_fails(
            self.source.replace(anchor, "macro_rules! compact { () => {} }", 1),
            "forbidden production token",
        )
        self.assert_source_mutation_fails(
            self.source.replace(anchor, 'include!("compacted_body.rs");', 1),
            "forbidden production token",
        )

    def test_asset_and_minification_mutations_fail(self) -> None:
        first_asset = "crates/kotodama_lang/src/ir/fixtures/v1/i001.ko"
        original = _regular_bytes(REPO_ROOT / first_asset, REPO_ROOT)
        with self.assertRaisesRegex(GuardError, "asset ledger"):
            _validate_candidate(
                self.source,
                self.public_leaf,
                self.tail_leaf,
                check_full_hash=False,
                asset_overrides={first_asset: original + b" "},
            )
        self.assert_source_mutation_fails(
            self.source.replace(
                "//! Intermediate representation for Kotodama programs.\n//!",
                "//! Intermediate representation for Kotodama programs. //!",
                1,
            ),
            "postimage LOC",
        )


if __name__ == "__main__":
    unittest.main()
