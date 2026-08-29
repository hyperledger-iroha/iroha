#!/usr/bin/env python3
"""Fail closed on the Halo2 backend shard-02 fixture compaction.

The guard authenticates the indexed opening blob and the landed postimage,
records both the historical and explicit constrained-pow5 test inventories,
and pins the shared proof builders and their caller partition.
"""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_core/src/zk/halo2_backend_02_tests.rs"

PREIMAGE_BLOB = "24d6dcc6c3d5aa718563bc05f872e5034f9108a9"
PREIMAGE_SHA256 = "2038f9e73c032bf40e6de658ed934946c515f1fd15484c382bc7174614c47c99"
PREIMAGE_LINES = 1_616
POSTIMAGE_BLOB = "f69c2f0458c7864d82a81d4d999a4ec45a884524"
POSTIMAGE_SHA256 = "ce8e43d0abb32848099f29ddd649abc2df698b82dc20fc733f17eb4d9c2bb56e"
POSTIMAGE_LINES = 960
MINIMUM_RUST_LINE_REDUCTION = 656
MAX_LINE_LENGTH = 100

PREIMAGE_TESTS = (
    "halo2_poseidon_commit_open_chip_ipa",
    "halo2_poseidon_merkle2_chip_ipa",
    "halo2_verify_vote_bool_commit_merkle8_ipa",
    "halo2_verify_anon_transfer_2x2_merkle8_ipa",
    "halo2_verify_vote_bool_commit_merkle8_poseidon_ipa",
    "halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_permutation_harness",
    "halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_truncated_prof",
    "halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1",
    "halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1",
    "halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_truncated_prof",
    "halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_noncanonical",
    "halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_invalid_header",
    "halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_permutation_harness",
    "halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_randomized_min",
    "halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_permutation_harness",
)
EXPECTED_TESTS = (
    "halo2_constrained_commit_open_ipa",
    "halo2_constrained_merkle2_ipa",
    "halo2_verify_vote_bool_commit_merkle8_ipa",
    "halo2_verify_anon_transfer_2x2_merkle8_ipa",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa",
    "retired_poseidon_backend_label_is_not_a_pow5_alias",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_permutation_harness",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_truncated_prof",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_truncated_prof",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_noncanonical",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_invalid_header",
    "halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_permutation_harness",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_randomized_min",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_permutation_harness",
)
WITH_INSTANCE_TESTS = (
    "halo2_constrained_commit_open_ipa",
    "halo2_constrained_merkle2_ipa",
    "halo2_verify_vote_bool_commit_merkle8_ipa",
    "halo2_verify_anon_transfer_2x2_merkle8_ipa",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa",
    "retired_poseidon_backend_label_is_not_a_pow5_alias",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1",
)
WITHOUT_INSTANCE_TESTS = (
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_truncated_prof",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_malformed_inst",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_truncated_prof",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_noncanonical",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_invalid_header",
)
DIRECT_TESTS = (
    "halo2_verify_vote_bool_commit_merkle8_pow5_ipa_zk1_permutation_harness",
    "halo2_verify_anon_transfer_2x2_merkle8_pow5_ipa_zk1_permutation_harness",
    "halo2_verify_vote_bool_commit_merkle16_pow5_ipa_zk1_randomized_min",
    "halo2_verify_anon_transfer_2x2_merkle16_pow5_ipa_zk1_permutation_harness",
)

RAW_STRING_START = re.compile(r'(?:b?r)(#*)"')
FORBIDDEN = re.compile(
    r"(?:dyn|impl)\s+Fn(?:Once|Mut)?\b|\bFn(?:Once|Mut)?\s*\(|"
    r"(?:type\s+[A-Za-z_]\w*\s*=|:|->)\s*fn\s*\(|"
    r"\b(?:struct|enum|type)\s+(?:Action|Step|Body|Assertion)\w*\b|"
    r"\$(?:body|setup|action|step|assertion)|macro_rules!|"
    r"include(?:_str|_bytes)?!\s*\(|#\s*\[\s*path\s*=|"
    r"#\s*\[\s*rustfmt::skip\s*\]"
)


class GuardError(AssertionError):
    """The compacted Halo2 shard no longer matches its audited contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(source: str) -> str:
    return hashlib.sha256(source.encode()).hexdigest()


def _git_blob(source: str) -> str:
    payload = source.encode()
    header = f"blob {len(payload)}\0".encode()
    return hashlib.sha1(header + payload).hexdigest()


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


def _read_source() -> str:
    _require(SOURCE_PATH.is_file(), "Halo2 shard source is missing")
    _require(not SOURCE_PATH.is_symlink(), "Halo2 shard source must not be a symlink")
    try:
        SOURCE_PATH.resolve(strict=True).relative_to(ROOT)
    except ValueError as error:
        raise GuardError("Halo2 shard source escapes the repository") from error
    return SOURCE_PATH.read_text(encoding="utf-8")


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


def _function(source: str, name: str) -> str:
    pattern = re.compile(
        rf"(?m)^[ \t]*(?:(?:pub(?:\([^\n)]*\))?)\s+)?"
        rf"(?:(?:async|const|unsafe)\s+)*fn\s+{re.escape(name)}"
        rf"(?:<[^{{\n]*>)?\s*\("
    )
    matches = tuple(pattern.finditer(source))
    _require(len(matches) == 1, f"expected one function named {name}")
    opening = source.find("{", matches[0].end())
    _require(opening >= 0, f"missing function body for {name}")
    return source[matches[0].start() : _matching_delimiter(source, opening) + 1]


def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    lines = source.splitlines(keepends=True)
    inventory: list[tuple[str, tuple[str, ...]]] = []
    cursor = 0
    while cursor < len(lines):
        if not re.match(r"^[ \t]*#\[", lines[cursor]):
            cursor += 1
            continue
        attributes: list[str] = []
        while cursor < len(lines) and re.match(r"^[ \t]*#\[", lines[cursor]):
            attribute: list[str] = []
            bracket_depth = 0
            while cursor < len(lines):
                line = lines[cursor]
                attribute.append(line.rstrip("\n"))
                bracket_depth += line.count("[") - line.count("]")
                cursor += 1
                if bracket_depth == 0:
                    break
            _require(bracket_depth == 0, "unterminated Rust attribute")
            attributes.append("\n".join(attribute).strip())
        if cursor >= len(lines):
            break
        function = re.match(
            r"^[ \t]*(?:async[ \t]+)?fn[ \t]+([A-Za-z_]\w*)[ \t]*\(",
            lines[cursor],
        )
        if function and "#[test]" in attributes:
            inventory.append((function.group(1), tuple(attributes)))
    return tuple(inventory)


def _compact(source: str) -> str:
    return re.sub(r"\s+", "", source)


def _require_order(source: str, anchors: tuple[str, ...], label: str) -> None:
    compact = _compact(source)
    cursor = 0
    for anchor in anchors:
        expected = _compact(anchor)
        index = compact.find(expected, cursor)
        _require(index >= 0, f"{label} sequence changed at {anchor!r}")
        cursor = index + len(expected)


def _without_direct_functions(source: str) -> str:
    audited = source
    for name in DIRECT_TESTS:
        function = _function(audited, name)
        audited = audited.replace(function, "\n" * function.count("\n"), 1)
    return audited


def validate_source(source: str, preimage: str) -> None:
    """Validate the compact shard and every behavior-preserving invariant."""

    _require(_git_blob(preimage) == PREIMAGE_BLOB, "preimage Git blob changed")
    _require(_sha256(preimage) == PREIMAGE_SHA256, "preimage SHA-256 changed")
    _require(len(preimage.splitlines()) == PREIMAGE_LINES, "preimage line count changed")
    _require(len(source.splitlines()) == POSTIMAGE_LINES, "postimage line count changed")
    _require(
        PREIMAGE_LINES - POSTIMAGE_LINES == MINIMUM_RUST_LINE_REDUCTION,
        "Halo2 shard reduction changed",
    )

    preimage_tests = _test_inventory(preimage)
    postimage_tests = _test_inventory(source)
    _require(
        tuple(name for name, _ in preimage_tests) == PREIMAGE_TESTS,
        "authenticated preimage test inventory changed",
    )
    _require(len(postimage_tests) == 18, "direct test count changed")
    _require(
        tuple(name for name, _ in postimage_tests) == EXPECTED_TESTS,
        "test identifiers or order changed",
    )

    partition = WITH_INSTANCE_TESTS + WITHOUT_INSTANCE_TESTS + DIRECT_TESTS
    _require(len(set(partition)) == 18, "test caller partition overlaps")
    _require(set(partition) == set(EXPECTED_TESTS), "test caller partition changed")
    _require(
        source.count("ipa_fixture::build_with_instances(") == 8,
        "build_with_instances caller count changed",
    )
    _require(
        source.count("ipa_fixture::build_without_instances(") == 6,
        "build_without_instances caller count changed",
    )
    for name in WITH_INSTANCE_TESTS:
        function = _function(source, name)
        _require(
            function.count("ipa_fixture::build_with_instances(") == 1
            and "ipa_fixture::build_without_instances(" not in function,
            f"with-instances caller changed: {name}",
        )
    for name in WITHOUT_INSTANCE_TESTS:
        function = _function(source, name)
        _require(
            function.count("ipa_fixture::build_without_instances(") == 1
            and "ipa_fixture::build_with_instances(" not in function,
            f"without-instances caller changed: {name}",
        )
    for name in DIRECT_TESTS:
        function = _function(source, name)
        _require(
            "ipa_fixture::build_with_instances(" not in function
            and "ipa_fixture::build_without_instances(" not in function,
            f"direct test was routed through a helper: {name}",
        )

    with_instances = _function(source, "build_with_instances")
    without_instances = _function(source, "build_without_instances")
    constructor = _function(source, "new")
    for label, builder in (
        ("with-instances builder", with_instances),
        ("without-instances builder", without_instances),
    ):
        _require(builder.count('.expect("vk")') == 1, f"{label} VK diagnostic changed")
        _require(builder.count('.expect("pk")') == 1, f"{label} PK diagnostic changed")
        _require(
            builder.count('.expect("proof created")') == 1,
            f"{label} proof diagnostic changed",
        )
        _require(builder.count("transcript.finalize()") == 1, f"{label} finalize changed")
    _require("&[&[]]" not in with_instances, "with-instances builder became empty")
    _require(
        without_instances.count("&[&[]]") == 1,
        "without-instances builder must use one empty instance column",
    )
    common_sequence = (
        "let params: PastaParams = pasta_params_new(k);",
        'keygen_vk(&params, &circuit).expect("vk")',
        'keygen_pk(&params, vk.clone(), &circuit).expect("pk")',
        "Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![])",
        "halo2_proofs::plonk::create_proof::<",
        "&params",
        "&pk",
        "&[circuit]",
    )
    _require_order(
        with_instances,
        common_sequence
        + (
            "&[instance_columns]",
            '.expect("proof created")',
            "IpaProofFixture::new(k, &vk, transcript.finalize())",
        ),
        "with-instances builder",
    )
    _require_order(
        without_instances,
        common_sequence
        + (
            "&[&[]]",
            '.expect("proof created")',
            "IpaProofFixture::new(k, &vk, transcript.finalize())",
        ),
        "without-instances builder",
    )
    _require_order(
        constructor,
        (
            "let mut vk_envelope = zk1::wrap_start();",
            "zk1::wrap_append_ipa_k(&mut vk_envelope, k);",
            "zk1::wrap_append_vk_pasta(&mut vk_envelope, vk);",
            "Self { vk_envelope, proof_bytes, }",
        ),
        "verifying-key envelope",
    )

    audited = _without_direct_functions(source)
    _require(not FORBIDDEN.search(audited), "forbidden callback, DSL, macro, or relocation")
    audited_lines = audited.splitlines()
    _require(not any("\t" in line for line in audited_lines), "tab minification detected")
    _require(
        max(map(len, audited_lines), default=0) <= MAX_LINE_LENGTH,
        "line packing detected outside protected direct tests",
    )

    _require(_git_blob(source) == POSTIMAGE_BLOB, "postimage Git blob changed")
    _require(_sha256(source) == POSTIMAGE_SHA256, "postimage SHA-256 changed")


def _replace_once(source: str, old: str, new: str) -> str:
    _require(source.count(old) == 1, f"mutation anchor changed: {old!r}")
    return source.replace(old, new, 1)


def _mutate_function(source: str, name: str, old: str, new: str) -> str:
    function = _function(source, name)
    _require(function.count(old) == 1, f"function mutation anchor changed: {name}")
    return source.replace(function, function.replace(old, new, 1), 1)


class Halo2Backend02CompactionSourceTest(unittest.TestCase):
    """Authenticate the shard and prove deliberate mutations fail closed."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = _read_source()
        cls.preimage = _blob(PREIMAGE_BLOB)

    def test_repository_source_contract(self) -> None:
        validate_source(self.source, self.preimage)

    def test_mutations_fail_closed(self) -> None:
        comment = "// Constrained Pow5 test circuits (IPA): commit-open and merkle2."
        mutations = {
            "test identity": _replace_once(
                self.source,
                "fn halo2_constrained_commit_open_ipa(",
                "fn halo2_constrained_commit_open_ipa_mutated(",
            ),
            "test attribute": _replace_once(
                self.source,
                "#[test]\nfn halo2_constrained_commit_open_ipa(",
                "#[ignore]\nfn halo2_constrained_commit_open_ipa(",
            ),
            "caller partition": self.source.replace(
                "ipa_fixture::build_with_instances(",
                "ipa_fixture::build_without_instances(",
                1,
            ),
            "empty instances": _replace_once(
                self.source, "&[&[]]", "&[&[Scalar::ZERO]]"
            ),
            "builder sequence": _mutate_function(
                self.source, "build_with_instances", '.expect("pk")', '.expect("key")'
            ),
            "protected direct test": _mutate_function(
                self.source,
                DIRECT_TESTS[0],
                "Step2::Unknown(0)",
                "Step2::Unknown(1)",
            ),
            "callback": _replace_once(self.source, comment, "type Hidden = fn();"),
            "action DSL": _replace_once(self.source, comment, "enum Action { Run }"),
            "macro packing": _replace_once(
                self.source, comment, "macro_rules! cases { () => {} }"
            ),
            "source relocation": _replace_once(
                self.source, comment, 'include!("hidden_cases.rs");'
            ),
            "line packing": _replace_once(self.source, comment, "// " + "x" * 101),
            "postimage digest": _replace_once(
                self.source, comment, "// Audited Halo2 fixture compaction."
            ),
            "line count": self.source + "\n// unexpected growth\n",
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label):
                self.assertNotEqual(mutated, self.source)
                with self.assertRaises(GuardError):
                    validate_source(mutated, self.preimage)

        mutated_preimage = self.preimage.replace(
            "fn halo2_poseidon_commit_open_chip_ipa(",
            "fn halo2_poseidon_commit_open_chip_ipa_mutated(",
            1,
        )
        with self.subTest(label="preimage authentication"):
            with self.assertRaises(GuardError):
                validate_source(self.source, mutated_preimage)


if __name__ == "__main__":
    unittest.main()
