#!/usr/bin/env python3
"""Guard the versioned FCMP source-contract assertion inventory.

The guard is read-only, requires only Python's standard library plus Git, and
authenticates the three historical Rust blobs whose literal assertion data was
moved into ``source_contract_groups_v1.json``.  It rejects asset/schema drift,
case relabeling, callback/body-DSL escape hatches, test-identity changes, and
growth beyond the formatted Rust-line ratchets.
"""

from __future__ import annotations

import ast
import hashlib
import json
import re
import subprocess
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TESTS = Path("crates/iroha_core/src/privacy_engines/fcmp_plus_plus/prover/tests.rs")
COMMITMENT = Path(
    "crates/iroha_core/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs"
)
RUNTIME = Path(
    "crates/iroha_core/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs"
)
PATHS = (TESTS, COMMITMENT, RUNTIME)
ASSET = Path(
    "crates/iroha_core/src/privacy_engines/fcmp_plus_plus/prover/tests/"
    "source_contract_groups_v1.json"
)
BLOBS = {
    TESTS: "4b6267d86f7bd45203188e665215c83eaa58b5f7",
    COMMITMENT: "cf118c236e6fc1fb88b70a7dcb6c66189e27c23a",
    RUNTIME: "e81723d0b934b2295c2ed6fd198aab880867c61c",
}
PREIMAGE_SHA256 = {
    TESTS: "cf940f65fb716c719d9dde7d095960a05e461e1aca62f0cc885c2e1fd7544e92",
    COMMITMENT: "ecaa6bc06fdfccc05c0f1969163de4740276939190224413cf08cd406828387c",
    RUNTIME: "f4bcb2537e212b9d36a94ac777b90b6947eb4b30b0e41f7db48c8422ce9bff15",
}
SOURCE_SHA256 = {
    TESTS: "d17a30ad92fc79259a0b0f91ead2ba9a2a7642598e9d1d85387e5ce640d7ac93",
    COMMITMENT: "b7ec4e254bfbbe37bf3aac024a276064128c11b0578938ff2bd1ec3721e302e8",
    RUNTIME: "1c872fcb5b810a41c18ca0635e4dc18e9728e845581ea7d08511b27c3ec65ccd",
}
LINE_CEILINGS = {TESTS: 2_696, COMMITMENT: 1_572, RUNTIME: 1_794}
ASSET_LEN = 40_117
ASSET_SHA256 = "102ca2e207e0560f7c679a7a5345f82a2204f70079c09086dd036d97489963d5"
GROUPS_SHA256 = "667f099c4128ddaf365f125483afaebc0a932383c3ef36fc2fe01cb82767e220"
IDS_SHA256 = "aa656881a03be347db4cb952bc39c62318c076fe3908b9fa71e6816230b369a2"
TESTS_SHA256 = "bc09853b45ab038492e4e2a8265d974425d273e0503f2df45e069522edf2f1e0"
LOADER_SHA256 = "9f3b43069c73682419d5e8cc63d9f72c07aea72fbbdf0c91aa8e8ed237d1bd39"
GROUP_COUNT = 76
KINDS = {
    "assert_source_contains_all": "contains",
    "assert_source_excludes_all": "excludes",
    "assert_source_order": "order",
    "assert_source_counts": "counts",
}
FORBIDDEN_LOADER_TOKENS = (
    "Fn(",
    "FnMut",
    "FnOnce",
    "Box<dyn",
    "callback",
    "$body",
    "$setup",
    "Action",
    "Scenario",
    "Step",
)
TEST_PATTERN = re.compile(
    r"(?m)((?:^#\[[^\n]+\]\n)+)fn\s+([A-Za-z0-9_]+)\s*\("
)
CALL_PATTERN = re.compile(
    r"\b(assert_source_contains_all|assert_source_excludes_all|"
    r"assert_source_order|assert_source_counts)\s*\("
)
MIGRATED_PATTERN = re.compile(
    r'assert_source_contract_group\s*\(\s*"([^"]+)"'
)
FUNCTION_PATTERN = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?"
    r"fn[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*\("
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def git_blob(blob: str) -> bytes:
    """Read an authenticated historical blob without changing repository state."""

    return subprocess.run(
        ["git", "cat-file", "blob", blob],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout


def skip_quoted(source: str, index: int) -> int:
    """Return the first index after one Rust string, character, or raw literal."""

    raw = re.match(r"(?:b?r)(#*)\"", source[index:])
    if raw:
        terminator = '"' + raw.group(1)
        end = source.find(terminator, index + raw.end())
        if end < 0:
            raise AssertionError("unterminated Rust raw string")
        return end + len(terminator)
    quote_index = index + (1 if source.startswith("b\"", index) else 0)
    quote = source[quote_index]
    cursor = quote_index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    raise AssertionError("unterminated Rust quoted literal")


def matching_delimiter(source: str, start: int, opener: str, closer: str) -> int:
    """Find a matching Rust delimiter while ignoring literals and comments."""

    if source[start] != opener:
        raise AssertionError(f"expected {opener!r} at {start}")
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
                raise AssertionError("unterminated Rust block comment")
            continue
        if source[cursor] == '"' or source.startswith(('b"', 'r"', 'br"'), cursor):
            cursor = skip_quoted(source, cursor)
            continue
        raw_prefix = re.match(r"(?:b?r)#+\"", source[cursor:])
        if raw_prefix:
            cursor = skip_quoted(source, cursor)
            continue
        if source[cursor] == "'" and cursor + 2 < len(source):
            closing = cursor + 2 if source[cursor + 1] != "\\" else cursor + 3
            if closing < len(source) and source[closing] == "'":
                cursor = closing + 1
                continue
        if source[cursor] == opener:
            depth += 1
        elif source[cursor] == closer:
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise AssertionError(f"unterminated {opener}{closer} delimiter")


def mask_non_code(source: str) -> str:
    """Blank literals and comments while preserving byte-compatible text offsets."""

    masked = list(source)
    cursor = 0
    while cursor < len(source):
        end = None
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            end = len(source) if newline < 0 else newline
        elif source.startswith("/*", cursor):
            comment_depth = 1
            end = cursor + 2
            while end < len(source) and comment_depth:
                if source.startswith("/*", end):
                    comment_depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    comment_depth -= 1
                    end += 2
                else:
                    end += 1
            if comment_depth:
                raise AssertionError("unterminated Rust block comment")
        elif source[cursor] == '"' or source.startswith(('b"', 'r"', 'br"'), cursor):
            end = skip_quoted(source, cursor)
        elif re.match(r"(?:b?r)#+\"", source[cursor:]):
            end = skip_quoted(source, cursor)
        elif source[cursor] == "'" and cursor + 2 < len(source):
            closing = cursor + 2 if source[cursor + 1] != "\\" else cursor + 3
            if closing < len(source) and source[closing] == "'":
                end = closing + 1
        if end is None:
            cursor += 1
            continue
        for index in range(cursor, end):
            if masked[index] != "\n":
                masked[index] = " "
        cursor = end
    return "".join(masked)


def split_top_level(source: str, separator: str = ",") -> list[str]:
    """Split a Rust token fragment only at top-level separators."""

    parts: list[str] = []
    start = 0
    cursor = 0
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            end = source.find("*/", cursor + 2)
            if end < 0:
                raise AssertionError("unterminated Rust block comment")
            cursor = end + 2
            continue
        if source[cursor] == '"' or source.startswith(('b"', 'r"', 'br"'), cursor):
            cursor = skip_quoted(source, cursor)
            continue
        raw_prefix = re.match(r"(?:b?r)#+\"", source[cursor:])
        if raw_prefix:
            cursor = skip_quoted(source, cursor)
            continue
        character = source[cursor]
        if character in pairs:
            stack.append(pairs[character])
        elif stack and character == stack[-1]:
            stack.pop()
        elif character == separator and not stack:
            parts.append(source[start:cursor].strip())
            start = cursor + 1
        cursor += 1
    tail = source[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def function_spans(source: str) -> list[tuple[int, int, str]]:
    """Return brace-bounded function spans needed to label donor calls."""

    spans = []
    masked = mask_non_code(source)
    for match in FUNCTION_PATTERN.finditer(masked):
        open_paren = masked.find("(", match.start())
        close_paren = matching_delimiter(source, open_paren, "(", ")")
        body = source.find("{", close_paren)
        semicolon = source.find(";", close_paren)
        if body < 0 or (0 <= semicolon < body):
            continue
        end = matching_delimiter(source, body, "{", "}")
        spans.append((match.start(), end + 1, match.group(1)))
    return spans


def rust_string(token: str) -> str:
    """Decode the ordinary Rust strings used by the frozen donor inventory."""

    token = token.strip()
    if not token.startswith('"'):
        raise AssertionError(f"non-literal source-contract needle: {token[:40]}")
    value = ast.literal_eval(token)
    if not isinstance(value, str):
        raise AssertionError("source-contract needle did not decode to text")
    return value


def donor_groups(path: Path, data: bytes) -> list[dict[str, object]]:
    """Reconstruct the exact literal groups migrated from one donor blob."""

    source = data.decode("utf-8")
    masked = mask_non_code(source)
    spans = function_spans(source)
    calls = []
    for match in CALL_PATTERN.finditer(masked):
        open_paren = masked.find("(", match.start())
        close_paren = matching_delimiter(source, open_paren, "(", ")")
        if source.count("\n", match.start(), close_paren) + 1 < 4:
            continue
        owners = [name for start, end, name in spans if start <= match.start() < end]
        if not owners:
            raise AssertionError(f"ownerless donor call in {path}")
        arguments = split_top_level(source[open_paren + 1 : close_paren])
        if len(arguments) != 2:
            raise AssertionError(f"unexpected donor call arguments in {path}")
        array_argument = arguments[1].strip()
        if not array_argument.startswith("&[") or not array_argument.endswith("]"):
            raise AssertionError(f"non-array donor inventory in {path}")
        elements = split_top_level(array_argument[2:-1])
        needles: list[str] = []
        counts: list[int] = []
        if match.group(1) == "assert_source_counts":
            for element in elements:
                if not element.startswith("(") or not element.endswith(")"):
                    raise AssertionError("malformed donor count tuple")
                needle, count = split_top_level(element[1:-1])
                needles.append(rust_string(needle))
                counts.append(int(count.replace("_", ""), 0))
        else:
            needles = [rust_string(element) for element in elements]
        calls.append((match.start(), owners[-1], match.group(1), needles, counts))
    calls.sort()
    ordinals: dict[str, int] = {}
    groups = []
    for _, owner, function, needles, counts in calls:
        ordinal = ordinals.get(owner, 0)
        ordinals[owner] = ordinal + 1
        groups.append(
            {
                "id": f"{owner}/{ordinal:02d}",
                "kind": KINDS[function],
                "needles": needles,
                "counts": counts,
            }
        )
    return groups


def collect_test_inventory(source_map: dict[Path, bytes]) -> list[str]:
    """Return ordered test attributes and names across the three files."""

    rows = []
    for path in PATHS:
        source = source_map[path].decode("utf-8")
        for attributes, name in TEST_PATTERN.findall(source):
            if "#[test]\n" in attributes:
                rows.append(f"{path}:{attributes.replace(chr(10), '|')}{name}")
    return rows


def validate_snapshot(
    source_map: dict[Path, bytes], asset_bytes: bytes, *, enforce_pins: bool
) -> None:
    """Validate the current projection against authenticated donor semantics."""

    if enforce_pins:
        assert len(asset_bytes) == ASSET_LEN
        assert sha256(asset_bytes) == ASSET_SHA256
        for path in PATHS:
            assert sha256(source_map[path]) == SOURCE_SHA256[path]
    fixture = json.loads(asset_bytes)
    assert set(fixture) == {"schema", "preimage", "groups"}
    assert fixture["schema"] == "iroha_core.fcmp_source_contract_groups.v1"
    assert fixture["preimage"] == {
        "tests_rs": BLOBS[TESTS],
        "commitment_mask_rs": BLOBS[COMMITMENT],
        "runtime_rs": BLOBS[RUNTIME],
    }
    groups = fixture["groups"]
    assert len(groups) == GROUP_COUNT
    canonical = json.dumps(
        groups, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    assert sha256(canonical) == GROUPS_SHA256
    ids = [group["id"] for group in groups]
    assert len(ids) == len(set(ids))
    assert sha256(("\n".join(ids) + "\n").encode()) == IDS_SHA256
    donors = {}
    expected_groups = []
    for path in PATHS:
        donor = git_blob(BLOBS[path])
        assert sha256(donor) == PREIMAGE_SHA256[path]
        donors[path] = donor
        expected_groups.extend(donor_groups(path, donor))
    assert groups == expected_groups
    current_ids = []
    for path in PATHS:
        current_ids.extend(MIGRATED_PATTERN.findall(source_map[path].decode("utf-8")))
        assert source_map[path].count(b"\n") <= LINE_CEILINGS[path]
    assert current_ids == ids
    assert collect_test_inventory(source_map) == collect_test_inventory(donors)
    inventory = "\n".join(collect_test_inventory(source_map)) + "\n"
    assert sha256(inventory.encode()) == TESTS_SHA256
    tests_source = source_map[TESTS]
    loader_start = tests_source.index(b"const SOURCE_CONTRACT_GROUPS_V1:")
    loader_end = tests_source.index(
        b"#[derive(Clone, Copy)]\nenum SourcePoint", loader_start
    )
    loader = tests_source[loader_start:loader_end]
    assert sha256(loader) == LOADER_SHA256
    loader_text = loader.decode("utf-8")
    for token in FORBIDDEN_LOADER_TOKENS:
        assert token not in loader_text


def current_sources() -> dict[Path, bytes]:
    """Read the guarded worktree sources."""

    return {path: (REPO / path).read_bytes() for path in PATHS}


class FcmpSourceContractGroupsSourceTest(unittest.TestCase):
    """Exercise the source/asset contract and representative fail-closed mutations."""

    def test_current_projection_matches_authenticated_donors(self) -> None:
        validate_snapshot(current_sources(), (REPO / ASSET).read_bytes(), enforce_pins=True)

    def test_mutations_fail_closed(self) -> None:
        sources = current_sources()
        asset = (REPO / ASSET).read_bytes()
        mutations: list[tuple[dict[Path, bytes], bytes]] = []

        changed_asset = asset.replace(b'"kind": "contains"', b'"kind": "excludes"', 1)
        mutations.append((sources, changed_asset))
        changed_asset = asset.replace(b'"counts": []', b'"counts": [1]', 1)
        mutations.append((sources, changed_asset))
        changed_asset = asset.replace(BLOBS[TESTS].encode(), b"0" * 40, 1)
        mutations.append((sources, changed_asset))

        first_id = MIGRATED_PATTERN.search(sources[TESTS].decode("utf-8"))
        assert first_id is not None
        changed = dict(sources)
        changed[TESTS] = sources[TESTS].replace(first_id.group(1).encode(), b"renamed/00", 1)
        mutations.append((changed, asset))

        changed = dict(sources)
        changed[COMMITMENT] = sources[COMMITMENT].replace(
            b"#[test]", b"#[test]\n#[ignore = \"mutation\"]", 1
        )
        mutations.append((changed, asset))

        changed = dict(sources)
        changed[TESTS] = sources[TESTS].replace(
            b"fn assert_source_contract_group(id: &str, source: &str) {",
            b"fn assert_source_contract_group(id: &str, source: &str) {\n    // callback mutation",
            1,
        )
        mutations.append((changed, asset))

        changed = dict(sources)
        changed[RUNTIME] = sources[RUNTIME] + b"\n" * 2
        mutations.append((changed, asset))

        for index, (mutated_sources, mutated_asset) in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises((AssertionError, KeyError)):
                validate_snapshot(mutated_sources, mutated_asset, enforce_pins=False)


if __name__ == "__main__":
    unittest.main()
