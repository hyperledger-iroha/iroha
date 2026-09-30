#!/usr/bin/env python3
"""Guard current FCMP source-contract assets, direct assertions and owner wiring.

The active guard binds exact asset bytes to the current Rust consumer and rejects
missing groups, malformed data, callback interpreters and assertion bypasses.
"""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TESTS = Path("crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs")
COMMITMENT = Path(
    "crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs"
)
RUNTIME = Path(
    "crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs"
)
PATHS = (TESTS, COMMITMENT, RUNTIME)
ASSET = Path(
    "crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/"
    "source_contract_groups_v1.json"
)
BLOBS = {
    TESTS: "4b6267d86f7bd45203188e665215c83eaa58b5f7",
    COMMITMENT: "cf118c236e6fc1fb88b70a7dcb6c66189e27c23a",
    RUNTIME: "e81723d0b934b2295c2ed6fd198aab880867c61c",
}
GROUP_COUNT = 76
EXPECTED_TEST_INVENTORY = ['crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|prover_copy_owner_clears_transfer_success_and_unwind_slots',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_spendable_output_owns_inputs_and_secret_outputs_on_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_spendable_output_source_stays_owned_through_release_transfer',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_u64_wrapper_owns_slots_on_success_error_and_inner_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_u64_wrapper_source_takes_every_slot_before_inner_conversion',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_output_opening_owns_success_error_mismatch_and_unwind_slots',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_output_opening_source_stays_owned_until_borrowed_constructor',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_rerandomization_owns_success_error_and_unwind_slots',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|rerandomization_scalar_decoder_owns_comparison_wide_and_result_on_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|rerandomization_constructor_direct_handoff_covers_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_rerandomization_source_keeps_feature_secret_owners_in_order',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_leaf_coordinate_scope_owns_success_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_secret_selene_hash_matches_equation_and_owns_all_exit_paths',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_secret_cycle_step_matches_public_equations_and_owns_copies',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_secret_branch_direct_handoff_covers_capacity_success_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_secret_cycle_source_has_no_raw_coordinate_hash_or_branch_boundary',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|invalid_path_fixture_replacement_owns_success_error_and_zeroize_slots',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|invalid_path_fixture_replacement_final_owner_zeroizes_on_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|invalid_path_fixture_source_confines_both_replacements_to_direct_owner_swaps',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|root_value_equality_owns_every_coordinate_difference_and_scans_full_shape',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|root_value_equality_source_uses_only_borrowed_subtraction_and_owned_differences',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_leaf_coordinate_buffer_zeroizes_on_drop_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_leaf_coordinate_source_keeps_exact_erasing_owners_through_hash',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|hidden_output_identifier_push_is_preallocated_and_owned_on_success_and_error',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|private_output_identifier_callsites_use_only_borrowed_owned_insertion',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|duplicate_key_image_precheck_owners_cover_success_decode_error_capacity_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|duplicate_key_image_precheck_source_is_borrowed_owned_and_constant_time',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|proof_input_coordinate_owners_cover_success_decode_error_downstream_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|input_blind_v_padding_owner_covers_success_downstream_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_y_sum_owner_covers_success_constructor_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_linking_bytes_owner_covers_success_constructor_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_scalar_encoding_handoff_covers_decode_zeroize_downstream_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_linking_bytes_source_owns_encoding_through_constructor',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_spend_x_bytes_owner_covers_success_constructor_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_spend_x_bytes_source_owns_encoding_through_constructor',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_rerandomization_blind_bytes_owner_covers_success_constructor_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_rerandomization_blind_bytes_source_owns_encoding_through_constructor',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|sal_y_sum_source_borrows_operands_and_retains_owners_through_constructor',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|proof_input_coordinate_source_is_borrowed_owned_and_production_visible',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|fixture_secret_selene_hash_source_uses_borrowed_exact_builder_and_owned_result',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests.rs:#[test]|rerandomization_constructor_takes_all_bytes_before_decoding',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs:#[test]|prover_input_constructor_takes_secret_bytes_before_validation',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs:#[test]|prover_input_scalar_owner_handoff_covers_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs:#[test]|public_input_private_point_owners_cover_success_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs:#[test]|public_input_keeps_private_products_in_borrowed_erasing_owners',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/commitment_mask.rs:#[test]|commitment_mask_openings_remain_borrowed_until_the_membership_boundary',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prover_witness_debug_is_redacted_and_explicit_zeroize_covers_the_full_path',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|constant_work_scan_primitives_visit_every_element_and_pair',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|typed_membership_and_duplicate_scans_cover_every_position',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|hidden_leaf_membership_and_duplicates_cover_first_middle_last_and_absent',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|shared_root_scan_covers_first_middle_last_and_absent_mismatches',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|private_push_guard_forbids_vector_growth',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|maximum_compiled_shape_has_canonical_paths_and_exact_resource_bound',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|parse_path_private_owners_cover_success_error_and_unwind',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|secret_root_comparison_owns_encoding_on_match_mismatch_and_error',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|parse_path_source_keeps_private_values_in_owned_borrowed_order',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|malicious_zero_rng_exhausts_a_fixed_bound_instead_of_hanging',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|borrowed_path_coordinate_handoff_preflights_and_keeps_allocation_stable',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|owned_secret_scalar_handoff_keeps_preallocation_and_clears_source_on_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|negated_scalar_owner_handoff_retains_source_and_clears_every_temporary',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|sampled_scalar_slots_are_owned_before_rejection_or_return',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prepared_cycle_blind_owners_survive_handoff_until_success_drop',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prepared_cycle_blind_identity_coordinates_fail_without_unwrapping_owners',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prepared_cycle_blind_owners_clear_on_downstream_error_for_both_curves',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prepared_cycle_blind_owners_clear_on_unwind_for_both_curves',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|root_nonce_commitment_encoding_clears_both_point_owners_on_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|root_blind_response_encoding_clears_both_nonce_owners_on_every_exit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|membership_prover_retries_only_prover_honest_aborts_at_a_fixed_bound',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|#[ignore '
 '= "manual release resource audit; run under `/usr/bin/time -l` for peak '
 'RSS"]|maximum_compiled_shape_release_resource_audit',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|membership_rng_unavailability_fails_without_calling_infallible_rng_methods',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|public_prover_rejects_unavailable_and_short_period_entropy_before_proving',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|deterministic_preflight_errors_take_precedence_over_entropy_failure',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|extracted_prover_test_module_retains_every_legacy_regression',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|native_one_layer_prover_round_trips_end_to_end',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|native_two_layer_prover_exercises_alternating_curve_path',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|native_two_input_prover_round_trips_at_the_compiled_bound',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prover_rejects_duplicate_outputs_key_images_and_input_overflow_preflight',
 'crates/iroha_core_privacy/src/privacy_engines/fcmp_plus_plus/prover/tests/runtime.rs:#[test]|prover_paths_reject_reordered_omitted_and_duplicated_layers']
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
MIGRATED_PATTERN = re.compile(
    r'assert_source_contract_group\s*\(\s*"([^"]+)"'
)
FUNCTION_PATTERN = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?"
    r"fn[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*\("
)






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










def collect_test_inventory(source_map: dict[Path, bytes]) -> list[str]:
    """Return ordered test attributes and names across the three files."""

    rows = []
    for path in PATHS:
        source = source_map[path].decode("utf-8")
        for attributes, name in TEST_PATTERN.findall(source):
            if "#[test]\n" in attributes:
                rows.append(f"{path}:{attributes.replace(chr(10), '|')}{name}")
    return rows


def validate_current(
    source_map: dict[Path, bytes], asset_bytes: bytes
) -> None:
    """Validate current compile-time asset ownership and direct assertion wiring."""
    fixture = json.loads(asset_bytes)
    assert set(fixture) == {"schema", "preimage", "groups"}
    assert fixture["schema"] == "iroha_core.fcmp_source_contract_groups.v1"
    assert fixture["preimage"] == {
        "tests_rs": BLOBS[TESTS], "commitment_mask_rs": BLOBS[COMMITMENT],
        "runtime_rs": BLOBS[RUNTIME],
    }
    tests_source = source_map[TESTS].decode("utf-8")
    length = re.search(r"const SOURCE_CONTRACT_GROUPS_V1_LEN: usize = ([0-9_]+);", tests_source)
    digest = re.search(r"const SOURCE_CONTRACT_GROUPS_V1_SHA256: \[u8; 32\] = \[(.*?)\];", tests_source, re.DOTALL)
    assert length is not None and digest is not None
    assert len(asset_bytes) == int(length.group(1).replace("_", ""))
    declared_hash = bytes(int(value, 16) for value in re.findall(r"0x([0-9a-f]{2})", digest.group(1)))
    assert hashlib.sha256(asset_bytes).digest() == declared_hash
    groups = fixture["groups"]
    assert len(groups) == GROUP_COUNT
    ids = []
    for group in groups:
        assert set(group) == {"id", "kind", "needles", "counts"}
        assert isinstance(group["id"], str) and group["id"]
        assert group["kind"] in {"contains", "excludes", "order", "counts"}
        assert group["needles"] and all(isinstance(needle, str) and needle for needle in group["needles"])
        assert all(type(count) is int and count >= 0 for count in group["counts"])
        assert len(group["counts"]) == (len(group["needles"]) if group["kind"] == "counts" else 0)
        ids.append(group["id"])
    assert len(ids) == len(set(ids))
    current_ids = []
    for path in PATHS:
        source = source_map[path].decode("utf-8")
        current_ids.extend(MIGRATED_PATTERN.findall(source))
    assert current_ids == ids
    assert collect_test_inventory(source_map) == EXPECTED_TEST_INVENTORY
    function_names = {
        match.group(1)
        for source in source_map.values()
        for match in FUNCTION_PATTERN.finditer(mask_non_code(source.decode("utf-8")))
    }
    assert all(group_id.rsplit("/", 1)[0] in function_names for group_id in ids)
    assert tests_source.count("assert_sal_scalar_encoding_owner_handoff_source(") == 5
    loader_start = tests_source.index("const SOURCE_CONTRACT_GROUPS_V1:")
    loader_end = tests_source.index("#[derive(Clone, Copy)]\nenum SourcePoint", loader_start)
    loader = tests_source[loader_start:loader_end]
    for token in FORBIDDEN_LOADER_TOKENS:
        assert token not in loader
    for contract in (
        '#[norito(deny_unknown_fields)]',
        'assert_eq!(digest, SOURCE_CONTRACT_GROUPS_V1_SHA256)',
        'ids.insert(group.id.as_str())',
        'group.counts.len(), group.needles.len()',
        'assert!(source.contains(needle)', 'assert!(!source.contains(needle)',
        'source[cursor..]', 'cursor += offset + needle.len()',
        'source.matches(needle).count()', '*count',
        'unreachable!("source-contract kinds are validated at load")',
    ):
        assert contract in loader


def current_sources() -> dict[Path, bytes]:
    """Read the guarded worktree sources."""

    return {path: (REPO / path).read_bytes() for path in PATHS}


class FcmpSourceContractGroupsSourceTest(unittest.TestCase):
    """Exercise the source/asset contract and representative fail-closed mutations."""

    def test_current_asset_matches_consumer_and_executable_owners(self) -> None:
        validate_current(current_sources(), (REPO / ASSET).read_bytes())

    def test_source_trivia_growth_preserves_all_semantic_controls(self) -> None:
        """Each owner accepts extra whitespace with its exact assertion inventory."""
        sources = current_sources()
        asset = (REPO / ASSET).read_bytes()
        inventory = collect_test_inventory(sources)
        for path in PATHS:
            with self.subTest(path=path):
                expanded = dict(sources)
                expanded[path] += b"\n" * 16_101
                self.assertGreater(expanded[path].count(b"\n"), 16_100)
                self.assertEqual(collect_test_inventory(expanded), inventory)
                validate_current(expanded, asset)

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

        for index, (mutated_sources, mutated_asset) in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises((AssertionError, KeyError)):
                validate_current(mutated_sources, mutated_asset)


if __name__ == "__main__":
    unittest.main()
