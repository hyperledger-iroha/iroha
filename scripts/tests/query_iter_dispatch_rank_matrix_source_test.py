#!/usr/bin/env python3
"""Protect the typed iterable-dispatch rank sorting test matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/smartcontracts/isi/query.rs"
MAX_SOURCE_LINES = 8_950

CORE_START = """    #[derive(Clone, Copy)]
    enum IterDispatchRankFixture"""
CORE_END = """    #[tokio::test]
    async fn iter_dispatch_accounts_sort_ties_stable_by_id"""
CORE_HASH = "4f9059090c56eb02a61edd2d0e47775d2c9103a2d628d1eb2075f1b9793c5d59"

DIRECT_TESTS = (
    "iter_dispatch_sorts_and_paginates_end_to_end",
    "iter_dispatch_erased_and_canonical_parity_for_domains",
    "iter_dispatch_erased_and_canonical_parity_for_assets",
    "iter_dispatch_erased_and_canonical_parity_for_nfts",
    "iter_dispatch_erased_and_canonical_parity_for_accounts",
    "iter_dispatch_erased_and_canonical_parity_for_block_headers",
    "iter_dispatch_sorts_desc_end_to_end",
    "iter_dispatch_nfts",
    "iter_dispatch_triggers_basic",
    "iter_dispatch_pagination_offset_limit",
    "iter_dispatch_offset_and_fetch_size_interplay",
    "iter_dispatch_accounts_and_asset_definitions",
    "iter_dispatch_accounts_sort_ties_stable_by_id",
    "iter_dispatch_asset_definitions_sort_ties_stable_by_id",
    "iter_dispatch_find_triggers_full",
    "iter_dispatch_assets_non_empty_and_contains_minted",
    "iter_dispatch_accounts_with_asset_parity_and_continue",
    "iter_dispatch_domains_ids_only_projection",
    "iter_dispatch_accounts_ids_only_projection",
    "iter_dispatch_asset_definitions_ids_only_projection",
    "iter_dispatch_nfts_ids_only_projection",
    "iter_dispatch_roles_ids_only_projection",
    "iter_dispatch_triggers_ids_only_projection",
)

MATRIX_CASES = {
    "iter_dispatch_accounts_sort_desc_end_to_end": (
        "run_account_rank_sort_case",
        "Sparse",
        "Desc",
        "0",
        "None",
        "None",
        "&[&[0,1,2]]",
    ),
    "iter_dispatch_asset_definitions_sort_desc": (
        "run_asset_definition_rank_sort_case",
        "Sparse",
        "Desc",
        "0",
        "None",
        "None",
        "&[&[1,0,2]]",
    ),
    "iter_dispatch_asset_definitions_sort_asc": (
        "run_asset_definition_rank_sort_case",
        "Sparse",
        "Asc",
        "0",
        "None",
        "None",
        "&[&[0,1,2]]",
    ),
    "iter_dispatch_accounts_sort_asc_end_to_end": (
        "run_account_rank_sort_case",
        "Sparse",
        "Asc",
        "0",
        "None",
        "None",
        "&[&[1,0,2]]",
    ),
    "iter_dispatch_accounts_sort_desc_batched": (
        "run_account_rank_sort_case",
        "Sparse",
        "Desc",
        "0",
        "None",
        "Some(nonzero!(2_u64))",
        "&[&[0,1],&[2]]",
    ),
    "iter_dispatch_asset_definitions_sort_desc_batched": (
        "run_asset_definition_rank_sort_case",
        "Sparse",
        "Desc",
        "0",
        "None",
        "Some(nonzero!(2_u64))",
        "&[&[1,0],&[2]]",
    ),
    "iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_asc": (
        "run_asset_definition_rank_sort_case",
        "Dense",
        "Asc",
        "1",
        "Some(nonzero!(2_u64))",
        "Some(nonzero!(1_u64))",
        "&[&[1],&[2]]",
    ),
    "iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_desc": (
        "run_asset_definition_rank_sort_case",
        "Dense",
        "Desc",
        "1",
        "Some(nonzero!(2_u64))",
        "Some(nonzero!(1_u64))",
        "&[&[1],&[0]]",
    ),
    "iter_dispatch_accounts_offset_and_fetch_size_interplay": (
        "run_account_rank_sort_case",
        "Dense",
        "Asc",
        "1",
        "Some(nonzero!(2_u64))",
        "Some(nonzero!(1_u64))",
        "&[&[1],&[2]]",
    ),
    "iter_dispatch_accounts_offset_and_fetch_size_interplay_desc": (
        "run_account_rank_sort_case",
        "Dense",
        "Desc",
        "1",
        "Some(nonzero!(2_u64))",
        "Some(nonzero!(1_u64))",
        "&[&[1],&[0]]",
    ),
}

EXPECTED_TEST_ORDER = (
    *DIRECT_TESTS[:12],
    "iter_dispatch_accounts_sort_desc_end_to_end",
    *DIRECT_TESTS[12:14],
    "iter_dispatch_asset_definitions_sort_desc",
    *DIRECT_TESTS[14:],
    "iter_dispatch_asset_definitions_sort_asc",
    "iter_dispatch_accounts_sort_asc_end_to_end",
    "iter_dispatch_accounts_sort_desc_batched",
    "iter_dispatch_asset_definitions_sort_desc_batched",
    "iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_asc",
    "iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_desc",
    "iter_dispatch_accounts_offset_and_fetch_size_interplay",
    "iter_dispatch_accounts_offset_and_fetch_size_interplay_desc",
)

PROJECTION_TESTS = frozenset(
    {
        "iter_dispatch_domains_ids_only_projection",
        "iter_dispatch_accounts_ids_only_projection",
        "iter_dispatch_asset_definitions_ids_only_projection",
        "iter_dispatch_nfts_ids_only_projection",
        "iter_dispatch_roles_ids_only_projection",
        "iter_dispatch_triggers_ids_only_projection",
    }
)

CORE_TOKENS = (
    "IterDispatchRankFixture::Sparse => [Some(2), Some(1), None]",
    "IterDispatchRankFixture::Dense => [Some(0), Some(1), Some(2)]",
    '["rose", "tulip", "peony"]',
    '["a0", "a1", "a2"]',
    "Pagination::new(limit, offset)",
    'sort_by_metadata_key: Some("rank".parse().unwrap())',
    "fetch_size: FetchSize::new(fetch_size)",
    "FindAccounts",
    "FindAssetsDefinitions",
    "QueryOutputBatchBox::$variant(values)",
    "assert_eq!(values.len(), expected_indices.len())",
    "assert_eq!(value.id(), &ids[*expected_position])",
    "assert_eq!(remaining, expected_remaining)",
    "assert_eq!(cursor.is_some(), has_next_page)",
    '.handle_iter_continue(',
    '#[tokio::test]\n            async fn $name()',
)

FORBIDDEN_CORE_TOKENS = ("$body", "$setup", "$assert", "Custom(", "FnMut", "dyn Fn")


class GuardError(AssertionError):
    """Raised when the protected iterable-dispatch matrix changes."""


def _normalize(source: str) -> str:
    return re.sub(r"\s+", "", source)


def _core_region(source: str) -> str:
    if source.count(CORE_START) != 1 or source.count(CORE_END) != 1:
        raise GuardError("rank-matrix region markers must occur exactly once")
    start = source.index(CORE_START)
    return source[start : source.index(CORE_END, start)]


def _expected_invocation(name: str, fields: tuple[str, ...]) -> str:
    return f"iter_dispatch_rank_sort_test!({name},{','.join(fields)});"


def _matrix_invocation(source: str, name: str) -> tuple[int, int, str]:
    pattern = re.compile(
        rf"    iter_dispatch_rank_sort_test!\(\s*{re.escape(name)},.*?\n    \);",
        re.DOTALL,
    )
    match = pattern.search(source)
    if match is None:
        raise GuardError(f"{name}: missing matrix invocation")
    return match.start(), match.end(), match.group()


def _direct_attributes(source: str, name: str) -> tuple[str, ...]:
    pattern = re.compile(
        rf"(?P<attrs>(?:    #\[[^\]\n]+\]\n)+)"
        rf"    async fn {re.escape(name)}\b"
    )
    match = pattern.search(source)
    if match is None:
        raise GuardError(f"{name}: missing direct async test")
    return tuple(re.findall(r"#\[([^\]\n]+)\]", match.group("attrs")))


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("query.rs exceeded the frozen rank-matrix source budget")

    core = _core_region(source)
    for token in CORE_TOKENS:
        if token not in core:
            raise GuardError(f"rank-matrix core missing semantic token {token!r}")
    for token in FORBIDDEN_CORE_TOKENS:
        if token in core:
            raise GuardError(f"rank-matrix core contains escape hatch {token!r}")
    observed_hash = hashlib.sha256(_normalize(core).encode()).hexdigest()
    if observed_hash != CORE_HASH:
        raise GuardError(f"rank-matrix core hash changed: {observed_hash}")

    expected_names = set(EXPECTED_TEST_ORDER)
    observed_names = set(
        re.findall(r"\biter_dispatch_[a-z0-9_]+\b", source)
    ) - {"iter_dispatch_rank_sort_test"}
    if observed_names != expected_names:
        raise GuardError(
            f"iterable-dispatch test names changed: {observed_names ^ expected_names}"
        )
    for name in EXPECTED_TEST_ORDER:
        if len(re.findall(rf"\b{re.escape(name)}\b", source)) != 1:
            raise GuardError(f"{name}: expected exactly one source occurrence")
    observed_order = tuple(sorted(expected_names, key=source.index))
    if observed_order != EXPECTED_TEST_ORDER:
        raise GuardError("iterable-dispatch test declaration order changed")

    normalized = _normalize(source)
    for name, fields in MATRIX_CASES.items():
        invocation = _expected_invocation(name, fields)
        if normalized.count(invocation) != 1:
            raise GuardError(f"{name}: typed matrix wiring changed")

    for name in DIRECT_TESTS:
        if name in PROJECTION_TESTS:
            expected = ('cfg(feature = "ids_projection")', "tokio::test")
        elif name == "iter_dispatch_accounts_with_asset_parity_and_continue":
            expected = ("tokio::test", "allow(clippy::too_many_lines)")
        else:
            expected = ("tokio::test",)
        observed = _direct_attributes(source, name)
        if observed != expected:
            raise GuardError(f"{name}: attributes {observed} != {expected}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class QueryIterDispatchRankMatrixSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def test_current_source_preserves_dispatch_matrix(self) -> None:
        validate_source(self.source)

    def test_name_mutation_is_rejected(self) -> None:
        name = "iter_dispatch_accounts_sort_desc_batched"
        mutated = _replace_once(self.source, name, f"{name}_mutated")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_ordered_attribute_mutation_is_rejected(self) -> None:
        name = "iter_dispatch_domains_ids_only_projection"
        old = f'#[cfg(feature = "ids_projection")]\n    #[tokio::test]\n    async fn {name}'
        mutated = _replace_once(self.source, old, old.replace("#[tokio::test]", "#[ignore]"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_sort_direction_mutation_is_rejected(self) -> None:
        name = "iter_dispatch_accounts_sort_asc_end_to_end"
        start, end, invocation = _matrix_invocation(self.source, name)
        changed = _replace_once(invocation, "\n        Asc,\n", "\n        Desc,\n")
        mutated = self.source[:start] + changed + self.source[end:]
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_expected_page_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "&[&[1, 0], &[2]]",
            "&[&[0, 1], &[2]]",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_fixture_rank_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "IterDispatchRankFixture::Dense => [Some(0), Some(1), Some(2)]",
            "IterDispatchRankFixture::Dense => [Some(0), Some(2), Some(1)]",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_continuation_assertion_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "assert_eq!(cursor.is_some(), has_next_page)",
            "assert!(cursor.is_some())",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
