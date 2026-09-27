"""Independent finite-rank and conditional-codec construction regressions."""

from pathlib import Path
from runpy import run_path

import pytest


C = run_path(str(Path(__file__).resolve().parents[1] / "check_deep_hiding_candidate.py"))


def test_full_closure_has_exact_rank_136_and_135_masks_are_insufficient() -> None:
    """Eight OOD constraints plus 128 distinct base constraints attain the bound."""
    matrix = C["opening_matrix"](list(range(64)), 136, (0, 1, 0, 0))
    assert len(matrix) == 136
    assert C["base_rank"](matrix) == 136
    assert C["base_rank"]([row[:135] for row in matrix]) == 135
    direct = C["opening_matrix"](list(range(64)), 136, (0, 1, 0, 0), False)
    assert C["base_rank"](direct) == 72


def test_query_overlap_and_quadratic_ood_have_smaller_rank_not_an_assumed_eight() -> None:
    """The policy is a worst-case bound; actual observations can be dependent."""
    indices = [i * 128 for i in range(64)]
    assert C["base_rank"](C["opening_matrix"](indices, 136, (0, 1, 0, 0))) == 73
    assert C["base_rank"](C["opening_matrix"](list(range(64)), 136, (0, 0, 1, 0))) == 132
    for indices in [[], [0, 0], [-1], [C["M"]]]:
        with pytest.raises(ValueError):
            C["opening_matrix"](indices, 136, (0, 1, 0, 0))
    with pytest.raises(ValueError):
        C["opening_matrix"]([0], 0, (0, 1, 0, 0))


def test_proposed_mask_field_charges_all_nested_framing_and_resources() -> None:
    """The composition-mask DTO remains distinct from a qualified private proof."""
    screen = C["candidate_screen"]()
    assert screen == {
        "witness_mask_base_coefficients": 136,
        "quotient_mask_fp4_coefficients": 65,
        "trace_degree_bound": 65_672,
        "numerator_degree_bound": 196_751,
        "quotient_degree_bound": 131_215,
        "randomized_low_degree_bound": 65_601,
        "randomized_high_degree_bound": 65_679,
        "pre_mask_candidate_frame_bytes": 500_783,
        "extra_framed_composition_mask_bytes": 2_112,
        "implemented_candidate_frame_bytes": 502_895,
        "candidate_margin_bytes": 21_393,
        "candidate_two_child_margin_before_carrier": 42_786,
        "witness_mask_payload_bytes": 327_488,
        "quotient_mask_payload_bytes": 2_080,
        "composition_mask_coefficient_payload_bytes": 4_194_304,
        "trace_coefficients_masks_and_one_stripe_payload_bytes": 315_948_864,
        "one_materialized_full_row_lde_payload_bytes": 20_199_768_064,
        "one_materialized_binary_digest_tree_payload_bytes": 805_306_320,
        "one_full_fp4_oracle_payload_bytes": 268_435_456,
    }
    C["check_source_frame"]()
    with pytest.raises(ValueError, match="current DEEP frame changed"):
        C["candidate_screen"](502_896)
    assert C["field"](127) == 128
    assert C["field"](128) == 130
    with pytest.raises(ValueError):
        C["field"](-1)
    for matrix in [[], [[]], [[1], [1, 2]]]:
        with pytest.raises(ValueError):
            C["base_rank"](matrix)
