"""Source contract for the shared private-note/PQ-MASP profile constraints."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SHARED_PATH = (
    REPO_ROOT
    / "crates"
    / "iroha_core"
    / "src"
    / "privacy_engines"
    / "shared_note_profile_constraints.rs"
)
INCLUDE = 'include!("../shared_note_profile_constraints.rs");'
ALIAS_NAMES = (
    "BASE_WIDTH",
    "PROFILE_AUX_WIDTH",
    "PROFILE_FIXED_WIDTH",
    "SHA_BIT_COLUMNS",
    "SHA_STATE_WORDS",
    "SHA_SCHEDULE_WORDS",
    "COPY_WIDTH",
    "DISTINCT_RIGHT_BITS_OFFSET",
    "VM_DIFFERENCE_BITS_OFFSET",
)
PROFILE_CONTRACTS = {
    "ivm": {
        "path": REPO_ROOT
        / "crates"
        / "iroha_core"
        / "src"
        / "privacy_engines"
        / "ivm_private_note"
        / "stark.rs",
        "function": "private_note_profile_constraint_residues_inner_v1",
        "occurrences": 7,
        "aliases": (
            ("BASE_WIDTH", "PRIVATE_NOTE_BASE_WIDTH_V1"),
            ("PROFILE_AUX_WIDTH", "PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1"),
            ("PROFILE_FIXED_WIDTH", "PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1"),
            ("SHA_BIT_COLUMNS", "PRIVATE_NOTE_SHA_BIT_COLUMNS_V1"),
            ("SHA_STATE_WORDS", "PRIVATE_NOTE_SHA_STATE_WORDS_V1"),
            ("SHA_SCHEDULE_WORDS", "PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1"),
            ("COPY_WIDTH", "PRIVATE_NOTE_COPY_WIDTH_V1"),
            ("DISTINCT_RIGHT_BITS_OFFSET", "SCRATCH_VM_DIFFERENCE_BITS_OFFSET"),
            ("VM_DIFFERENCE_BITS_OFFSET", "SCRATCH_VM_DIFFERENCE_BITS_OFFSET"),
        ),
        "expansion_sha256": (
            "8f7e5a2a7dfeeeabe7a687c881f41a1c7fda282d4b85ded727d863ec5e2c4388"
        ),
    },
    "pq": {
        "path": REPO_ROOT
        / "crates"
        / "iroha_core"
        / "src"
        / "privacy_engines"
        / "pq_masp"
        / "stark.rs",
        "function": "pq_masp_profile_constraint_residues_inner_v1",
        "occurrences": 8,
        "aliases": (
            ("BASE_WIDTH", "PQ_MASP_BASE_WIDTH_V1"),
            ("PROFILE_AUX_WIDTH", "PQ_MASP_PROFILE_AUX_WIDTH_V1"),
            ("PROFILE_FIXED_WIDTH", "PQ_MASP_PROFILE_FIXED_WIDTH_V1"),
            ("SHA_BIT_COLUMNS", "PQ_MASP_SHA_BIT_COLUMNS_V1"),
            ("SHA_STATE_WORDS", "PQ_MASP_SHA_STATE_WORDS_V1"),
            ("SHA_SCHEDULE_WORDS", "PQ_MASP_SHA_SCHEDULE_WORDS_V1"),
            ("COPY_WIDTH", "PQ_MASP_COPY_WIDTH_V1"),
            ("DISTINCT_RIGHT_BITS_OFFSET", "SCRATCH_DISTINCT_RIGHT_BITS_OFFSET"),
            ("VM_DIFFERENCE_BITS_OFFSET", "SCRATCH_VM_DIFFERENCE_BITS_OFFSET"),
        ),
        "expansion_sha256": (
            "82633c2e1ea3d9b534496930217bea27c7b226296ff206148aa43e0cc4ae7a78"
        ),
    },
}
EXPECTED_MACRO_SHA256 = (
    "4545062c5f68c546d6174cbfea8d445fb539c5e26f8236d5e55685c9219a8d9d"
)
TOKEN_PATTERN = re.compile(
    r"\$?[A-Za-z_][A-Za-z_0-9]*|\d+|::|\.\.|->|=>|==|!=|<=|>=|&&|\|\||"
    r"<<|>>|[-+*/%&|^!<>=.,;:(){}\[\]]"
)
ALIAS_PATTERN = re.compile(
    rf"^const ({'|'.join(ALIAS_NAMES)}): usize = ([A-Z][A-Z0-9_]+);$",
    re.MULTILINE,
)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _without_comments(source: str) -> str:
    source = re.sub(r"//[^\n]*", "", source)
    return re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)


def _token_hash(source: str) -> str:
    tokens = TOKEN_PATTERN.findall(_without_comments(source))
    return hashlib.sha256(" ".join(tokens).encode()).hexdigest()


def _function_template(shared: str) -> str:
    source = _without_comments(shared)
    marker = "fn $function_name("
    start = source.index(marker)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    raise AssertionError("shared profile-constraint function is unterminated")


def _alias_block(aliases: tuple[tuple[str, str], ...]) -> str:
    return "".join(f"const {name}: usize = {value};\n" for name, value in aliases)


def _validate_source(shared: str, profiles: dict[str, str]) -> None:
    _require(
        _token_hash(shared) == EXPECTED_MACRO_SHA256,
        "shared macro matcher, expansion, signature, or constraint order changed",
    )
    _require(
        shared.count("macro_rules! define_note_profile_constraint_residues_v1") == 1,
        "shared profile-constraint macro inventory changed",
    )
    template = _function_template(shared)
    for profile, contract in PROFILE_CONTRACTS.items():
        source = profiles[profile]
        function = str(contract["function"])
        aliases = contract["aliases"]
        assert isinstance(aliases, tuple)
        invocation = f"define_note_profile_constraint_residues_v1!({function});"
        exact_boundary = _alias_block(aliases) + INCLUDE + "\n" + invocation
        _require(
            source.count(exact_boundary) == 1,
            f"{profile} alias/include/invocation boundary changed",
        )
        _require(
            tuple(ALIAS_PATTERN.findall(source)) == aliases,
            f"{profile} profile alias values or order changed",
        )
        _require(
            f"fn {function}(" not in source,
            f"{profile} restored a second explicit constraint implementation",
        )
        occurrences = len(re.findall(rf"\b{re.escape(function)}\b", source))
        _require(
            occurrences == contract["occurrences"],
            f"{profile} constraint function call inventory changed",
        )
        expansion = template.replace("$function_name", function)
        for alias, value in sorted(aliases, key=lambda pair: -len(pair[0])):
            expansion = re.sub(rf"\b{alias}\b", value, expansion)
        _require(
            _token_hash(expansion) == contract["expansion_sha256"],
            f"{profile} expanded constraint tokens differ from the audited function",
        )


class NoteStarkProfileConstraintDedupSourceTests(unittest.TestCase):
    def test_shared_expansions_match_the_audited_profile_functions(self) -> None:
        shared = SHARED_PATH.read_text(encoding="utf-8")
        profiles = {
            profile: contract["path"].read_text(encoding="utf-8")
            for profile, contract in PROFILE_CONTRACTS.items()
        }
        _validate_source(shared, profiles)

    def test_contract_rejects_source_mutations(self) -> None:
        shared = SHARED_PATH.read_text(encoding="utf-8")
        profiles = {
            profile: contract["path"].read_text(encoding="utf-8")
            for profile, contract in PROFILE_CONTRACTS.items()
        }
        mutations = (
            (
                shared.replace("F::ONE.sub(allowed)", "F::ZERO.sub(allowed)", 1),
                profiles,
            ),
            (
                shared.replace("($function_name:ident)", "($function_name:path)", 1),
                profiles,
            ),
            (
                shared,
                {
                    **profiles,
                    "ivm": profiles["ivm"].replace(
                        "const DISTINCT_RIGHT_BITS_OFFSET: usize = "
                        "SCRATCH_VM_DIFFERENCE_BITS_OFFSET;",
                        "const DISTINCT_RIGHT_BITS_OFFSET: usize = "
                        "SCRATCH_VM_RESULT_BITS_OFFSET;",
                        1,
                    ),
                },
            ),
            (shared, {**profiles, "pq": profiles["pq"].replace(INCLUDE, "", 1)}),
            (
                shared,
                {
                    **profiles,
                    "ivm": profiles["ivm"].replace(
                        "define_note_profile_constraint_residues_v1!("
                        "private_note_profile_constraint_residues_inner_v1);",
                        "define_note_profile_constraint_residues_v1!("
                        "private_note_profile_constraint_residues_inner_v2);",
                        1,
                    ),
                },
            ),
        )
        for mutated_shared, mutated_profiles in mutations:
            with self.subTest():
                with self.assertRaises((AssertionError, ValueError)):
                    _validate_source(mutated_shared, mutated_profiles)


if __name__ == "__main__":
    unittest.main()
