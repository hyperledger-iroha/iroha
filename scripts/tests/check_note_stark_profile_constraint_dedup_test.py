"""Guard shared note constraints and the IVM membership wrapper using stdlib.

Shared arithmetic remains token-identical to the audited extraction. IVM adds
three positive-amount membership equalities around that shared implementation;
the production bridge must evaluate that wrapper. Test additions do not alter
the production call inventory.
"""

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
        "shared_function": "private_note_shared_constraint_residues_v1",
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
IVM_MEMBERSHIP_WRAPPER = """
fn private_note_profile_constraint_residues_inner_v1(
    current: &[F], next: &[F], current_aux: &[F], next_aux: &[F], fixed: &[F],
) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
    let mut residues =
        private_note_shared_constraint_residues_v1(current, next, current_aux, next_aux, fixed)?;
    let selector = fixed[NOTE_COPY_FIXED_WIDTH_V1 + TYPE_MEMBERSHIP];
    for pair in 0..3 {
        residues.push(
            selector
                .mul(current[COPY_OFFSET])
                .mul(current[COPY_OFFSET + 1 + pair * 2].sub(current[COPY_OFFSET + 2 + pair * 2])),
        );
    }
    Ok(residues)
}
"""
IVM_PRODUCTION_BRIDGE = """
fn constraint_residues_v1(
    self, current_base: &[F], next_base: &[F], current_aux: &[F],
    next_aux: &[F], fixed: &[F],
) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
    private_note_profile_constraint_residues_inner_v1(
        current_base, next_base, current_aux, next_aux, fixed,
    )
}
"""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _without_comments(source: str) -> str:
    source = re.sub(r"//[^\n]*", "", source)
    return re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)


def _token_hash(source: str) -> str:
    tokens = TOKEN_PATTERN.findall(_without_comments(source))
    return hashlib.sha256(" ".join(tokens).encode()).hexdigest()


def _function(source: str, name: str) -> str:
    source = _without_comments(source)
    marker = f"fn {name}("
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
    raise AssertionError(f"profile-constraint function {name} is unterminated")


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
    template = _function(shared, "$function_name")
    for profile, contract in PROFILE_CONTRACTS.items():
        source = profiles[profile]
        function = str(contract["function"])
        generated = str(contract.get("shared_function", function))
        aliases = contract["aliases"]
        assert isinstance(aliases, tuple)
        invocation = f"define_note_profile_constraint_residues_v1!({generated});"
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
            f"fn {generated}(" not in source,
            f"{profile} restored a second explicit constraint implementation",
        )
        production, marker, _tests = source.partition("#[cfg(test)]\nmod tests {")
        _require(bool(marker), f"{profile} test-module boundary changed")
        occurrences = len(re.findall(rf"\b{re.escape(generated)}\b", production))
        _require(
            occurrences == 2,
            f"{profile} generated constraint production call inventory changed",
        )
        if profile == "ivm":
            _require(
                _token_hash(_function(production, function)) == _token_hash(IVM_MEMBERSHIP_WRAPPER),
                "IVM positive-amount membership equalities or shared residues changed",
            )
            _require(
                _token_hash(_function(production, "constraint_residues_v1"))
                == _token_hash(IVM_PRODUCTION_BRIDGE),
                "IVM production relation bypasses the complete membership wrapper",
            )
            _require(
                len(re.findall(rf"\b{re.escape(function)}\b", production)) == 2,
                "IVM membership wrapper production call inventory changed",
            )
        # Use the audited function's name when fingerprinting its unchanged
        # arithmetic. The generated IVM name is now private to the wrapper.
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

    def test_additional_tests_do_not_change_production_call_inventory(self) -> None:
        shared = SHARED_PATH.read_text(encoding="utf-8")
        profiles = {
            profile: contract["path"].read_text(encoding="utf-8")
            for profile, contract in PROFILE_CONTRACTS.items()
        }
        for profile, contract in PROFILE_CONTRACTS.items():
            source = profiles[profile].replace(
                "mod tests {", f"mod tests {{\n// {contract['function']}", 1
            )
            _validate_source(shared, {**profiles, profile: source})

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
                        "private_note_shared_constraint_residues_v1);",
                        "define_note_profile_constraint_residues_v1!("
                        "private_note_profile_constraint_residues_inner_v2);",
                        1,
                    ),
                },
            ),
        )
        for mutated_shared, mutated_profiles in mutations:
            with self.subTest():
                self.assertTrue(mutated_shared != shared or mutated_profiles != profiles)
                with self.assertRaises((AssertionError, ValueError)):
                    _validate_source(mutated_shared, mutated_profiles)

    def test_membership_wrapper_mutations_fail_closed(self) -> None:
        shared = SHARED_PATH.read_text(encoding="utf-8")
        profiles = {
            profile: contract["path"].read_text(encoding="utf-8")
            for profile, contract in PROFILE_CONTRACTS.items()
        }
        ivm = profiles["ivm"]
        wrapper = _function(ivm, str(PROFILE_CONTRACTS["ivm"]["function"]))
        for old, new in (
            (".mul(current[COPY_OFFSET])", ""),
            ("for pair in 0..3", "for pair in 0..2"),
            ("TYPE_MEMBERSHIP", "TYPE_ADD"),
            ("COPY_OFFSET + 2 + pair * 2", "COPY_OFFSET + 1 + pair * 2"),
            ("Ok(residues)", "Ok(Vec::new())"),
        ):
            with self.subTest(old=old):
                self.assertIn(old, wrapper)
                mutated = ivm.replace(wrapper, wrapper.replace(old, new, 1), 1)
                self.assertNotEqual(mutated, ivm)
                with self.assertRaises(AssertionError):
                    _validate_source(shared, {**profiles, "ivm": mutated})
        bridge = _function(ivm, "constraint_residues_v1")
        bypass = bridge.replace(
            "private_note_profile_constraint_residues_inner_v1",
            "private_note_shared_constraint_residues_v1",
        )
        with self.assertRaises(AssertionError):
            _validate_source(shared, {**profiles, "ivm": ivm.replace(bridge, bypass, 1)})


if __name__ == "__main__":
    unittest.main()
