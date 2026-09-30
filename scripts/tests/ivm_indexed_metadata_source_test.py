#!/usr/bin/env python3
"""Freeze the typed IVM indexed-metadata rejection matrix."""

from __future__ import annotations

import hashlib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "crates/ivm/src/ivm.rs"
REGION_SHA256 = "0b09cf2e764dcc176ad7de50b472f6b5bfb5435191c35774e740a4ae35edb92e"
CASE_IDS = (
    "indexed_i64_out_of_range_is_rejected_at_load",
    "indexed_literal_opcode_kind_mismatch_is_rejected_at_load",
    "indexed_i64_rejects_unknown_kind_and_wrong_length",
    "indexed_literal_out_of_range_is_rejected_at_load",
    "indexed_literal_table_target_must_point_into_typed_data",
    "indexed_literal_table_rejects_duplicate_and_reordered_targets",
    "indexed_literal_table_rejects_gaps_interior_targets_and_bad_pointer_hashes",
)
REQUIRED_MATRIX_SNIPPETS = (
    "vec![program_with_indexed_i64(7, 1)]",
    "pointer_program[pointer_code..pointer_code + 4].copy_from_slice(&ldi64.to_le_bytes())",
    "scalar_program[scalar_code..scalar_code + 4].copy_from_slice(&ldlit.to_le_bytes())",
    "unknown_kind[descriptor + 7] = 0xff",
    "copy_from_slice(&1u32.to_le_bytes())",
    "copy_from_slice(&7u32.to_le_bytes())",
    "copy_from_slice(&9u32.to_le_bytes())",
    "copy_from_slice(&3u32.to_le_bytes())",
    "long.splice(data_end..data_end, [0; 4])",
    "vec![program_with_indexed_literal(1).0]",
    "copy_from_slice(&0u64.to_le_bytes())",
    "[[offsets[0], offsets[0]], [offsets[1], offsets[0]]]",
    "copy_from_slice(&(offsets[0] + 1).to_le_bytes())",
    "bad_hash[hash_byte] ^= 1",
    'assert_eq!(case_groups.len(), 7, "indexed admission case inventory")',
    "for (case_id, programs) in case_groups",
    "for program in programs",
    "Err(VMError::InvalidMetadata)",
    '"indexed admission case {case_id} did not fail closed"',
)
FORBIDDEN_MATRIX_SNIPPETS = (
    "Box<dyn Fn",
    "impl Fn",
    "dyn Fn",
    "fn(&",
    "$body",
    "$setup",
    "run_case",
    "custom_case",
)


class GuardError(AssertionError):
    """Raised when the source contract drifts."""


def _between(source: str, start: str, end: str) -> str:
    try:
        begin = source.index(start)
        finish = source.index(end, begin)
    except ValueError as error:
        raise GuardError(f"missing protected boundary: {error}") from error
    return source[begin:finish]


def _normalized_sha256(source: str) -> str:
    normalized = " ".join(source.split())
    return hashlib.sha256(normalized.encode()).hexdigest()


def check_source(source: str) -> None:
    """Validate the compacted Rust source without invoking the toolchain."""

    matrix = _between(
        source,
        "    fn code_hash_binds_indexed_i64_kind_and_payload()",
        "    fn pointer_validation_rejects_nonliteral_code_addresses()",
    )
    if _normalized_sha256(matrix) != REGION_SHA256:
        raise GuardError("indexed metadata matrix semantic seal changed")
    if "#[test]\n    fn indexed_program_metadata_rejections_are_fail_closed()" not in matrix:
        raise GuardError("typed indexed-metadata test lost its #[test] attribute")

    for case_id in CASE_IDS:
        if source.count(case_id) != 1:
            raise GuardError(f"historical case ID must occur exactly once: {case_id}")
        if case_id not in matrix:
            raise GuardError(f"historical case ID left the protected matrix: {case_id}")
    for snippet in REQUIRED_MATRIX_SNIPPETS:
        if snippet not in matrix:
            raise GuardError(f"missing indexed-metadata contract: {snippet}")
    for snippet in FORBIDDEN_MATRIX_SNIPPETS:
        if snippet in matrix:
            raise GuardError(f"opaque case escape hatch is forbidden: {snippet}")

    quiet_helper = """fn quiet_vm(gas_limit: u64) -> IVM {
        set_banner_enabled(false);
        IVM::new(gas_limit)
    }"""
    if source.count(quiet_helper) != 1:
        raise GuardError("quiet IVM fixture changed or duplicated")
    # The protected admission matrix creates each VM directly. Other VM tests
    # may reuse quiet_vm without changing this metadata contract.
    for constructor in ("set_banner_enabled(false);", "let mut vm = IVM::new(u64::MAX);"):
        if matrix.count(constructor) != 1:
            raise GuardError(f"indexed admission setup changed: {constructor}")

    code_hash_contracts = (
        "kind_mutation[metadata_len + 16 + 7] = 0",
        "payload_mutation[metadata_len + 16 + 8] ^= 1",
        "for mutation in [&kind_mutation, &payload_mutation]",
        "contract_code_hash(mutation)",
    )
    for contract in code_hash_contracts:
        if contract not in matrix:
            raise GuardError(f"indexed code-hash contract changed: {contract}")


class IndexedMetadataSourceTest(unittest.TestCase):
    """Exercise the source guard and representative adversarial mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = TARGET.read_text()

    def assert_rejected(self, source: str) -> None:
        check_source(self.source)
        self.assertNotEqual(source, self.source, "mutation did not change the fixture")
        with self.assertRaises(GuardError):
            check_source(source)

    def test_current_source(self) -> None:
        check_source(self.source)

    def test_missing_case_id_is_rejected(self) -> None:
        self.assert_rejected(self.source.replace(CASE_IDS[0], "missing", 1))

    def test_duplicate_case_id_is_rejected(self) -> None:
        self.assert_rejected(self.source.replace(CASE_IDS[1], CASE_IDS[0], 1))

    def test_error_category_drift_is_rejected(self) -> None:
        self.assert_rejected(
            self.source.replace(
                "matches!(vm.load_program(&program), Err(VMError::InvalidMetadata))",
                "matches!(vm.load_program(&program), Err(VMError::DecodeError))",
                1,
            )
        )

    def test_adversarial_hash_mutation_drift_is_rejected(self) -> None:
        self.assert_rejected(
            self.source.replace("bad_hash[hash_byte] ^= 1", "bad_hash[hash_byte] ^= 2", 1)
        )

    def test_callback_escape_hatch_is_rejected(self) -> None:
        marker = "for (case_id, programs) in case_groups"
        self.assert_rejected(self.source.replace(marker, "let _: Box<dyn Fn()>;\n        " + marker, 1))

    def test_quiet_fixture_body_drift_is_rejected(self) -> None:
        self.assert_rejected(self.source.replace("IVM::new(gas_limit)", "IVM::new(0)", 1))

    def test_unrelated_quiet_fixture_call_does_not_change_metadata_contract(self) -> None:
        anchor = "let mut vm = quiet_vm(100);"
        self.assertEqual(self.source.count(anchor), 1)
        check_source(self.source.replace(anchor, anchor + " let _other = quiet_vm(1);", 1))

    def test_whitespace_growth_preserves_metadata_contract(self) -> None:
        check_source(self.source + "\n" * 20_000)


if __name__ == "__main__":
    unittest.main()
