"""Exact diagnostic sizing checks for the fixed-row FASTPQ candidate."""

from itertools import combinations
from pathlib import Path
from runpy import run_path

import pytest


SCREEN = run_path(
    str(Path(__file__).resolve().parents[1] / "check_compact_geometry_screen.py")
)


def test_hypothetical_q375_fixed_row_frame_keeps_exact_counterexample() -> None:
    """The diagnostic must not regress to the retired variable-row formula."""
    current = SCREEN["hypothetical_frame"](375, 342, 524_288, 4, (2,) * 17)
    assert current["frame_bytes"] == 4_017_376
    assert current["fri_value_bytes"] == 272_512
    assert current["fri_sibling_bytes"] == 875_760


def test_exact_frontier_depends_on_positions() -> None:
    """A clustered query set needs fewer siblings than a spread set."""
    exact_frontier = SCREEN["exact_frontier"]
    assert exact_frontier(8, {0, 1}) == 2
    assert exact_frontier(8, {0, 7}) == 4


def test_exact_frontier_matches_explicit_siblings_for_every_small_subset() -> None:
    """Cross-check the parent-count identity against direct Merkle paths."""
    exact_frontier = SCREEN["exact_frontier"]
    for leaves in (2, 4, 8):
        for count in range(1, leaves + 1):
            for indices in combinations(range(leaves), count):
                opened = set(indices)
                siblings = set()
                for level in range(leaves.bit_length() - 1):
                    siblings.update(
                        (level, index ^ 1)
                        for index in opened
                        if index ^ 1 not in opened
                    )
                    opened = {index // 2 for index in opened}
                assert exact_frontier(leaves, set(indices)) == len(siblings)


def test_arity_only_screen_exceeds_the_segment_cap_for_an_exact_query_set() -> None:
    """No power-of-two fold partition fixes this Merkle-opening byte gap."""
    positions = tuple(index * 524_288 // 375 for index in range(375))
    minimum = SCREEN["min_strided_merkle_fri_bytes"]
    assert len(set(positions)) == 375
    assert minimum(positions, 524_288, 4, 17) == (556_496, (3, 3, 3, 8))
    assert minimum(positions, 524_288, 4, 3)[0] == 561_104
    assert 556_496 - SCREEN["CAP"] == 32_208


def test_all_fold_boundaries_confirm_the_two_arity_floors() -> None:
    """Enumerate all 65,536 partitions, including a separately bounded arity-16 screen."""
    positions = SCREEN["spread_queries"]
    exhaustive = SCREEN["exhaustive_strided_merkle_fri_bytes"]
    optimum = SCREEN["min_strided_merkle_fri_bytes"]
    unrestricted = exhaustive(positions, 524_288, 4, 17)
    implemented = exhaustive(positions, 524_288, 4, 4)
    assert unrestricted == (556_496, (3, 3, 3, 8), 1, 65_536)
    assert implemented == (558_544, (3, 3, 3, 4, 4), 1, 39_648)
    assert unrestricted[:2] == optimum(positions, 524_288, 4, 17)
    assert implemented[:2] == optimum(positions, 524_288, 4, 4)
    assert implemented[0] - SCREEN["CAP"] == 34_256


def test_best_arity16_hypothetical_schedule_charges_exact_round_components() -> None:
    """All 558,544 bytes precede roots, framing, AIR rows and other fields."""
    positions = SCREEN["spread_queries"]
    round_bytes = SCREEN["strided_fri_round_bytes"]
    start = 0
    charges = []
    for power in (3, 3, 3, 4, 4):
        charges.append(round_bytes(positions, 524_288, start, power))
        start += power
    assert charges == [
        (96_000, 132_576, 375),
        (96_000, 78_576, 375),
        (96_000, 24_576, 375),
        (32_768, 0, 64),
        (2_048, 0, 4),
    ]
    assert sum(value + frontier for value, frontier, _ in charges) == 558_544


def test_narrow_hypothetical_fixed_row_sizes_remain_nonqualifying() -> None:
    """The byte-only narrow screens retain their corrected framing."""
    frame = SCREEN["hypothetical_frame"]
    assert frame(64, 32, 8_388_608, 4, (8,) * 7)["frame_bytes"] == 469_093
    assert frame(72, 32, 8_388_608, 4, (8,) * 7)["frame_bytes"] == 519_326


def test_base_field_hiding_degree_and_exact_inline_row_mask_screen() -> None:
    """The candidate must retain its strict degree and both unchanged byte margins."""
    screen = SCREEN["base_field_hiding_degree_screen"]
    expected = {
        "trace_degree_bound": 98_304,
        "numerator_degree_bound": 262_015,
        "quotient_degree_bound": 196_479,
        "high_quotient_degree_bound": 130_943,
        "terminal_degree_bound": 2,
        "row_mask_added_bytes": 2_048,
        "frame_bytes": 502_831,
        "segment_margin_bytes": 21_457,
        "axt_two_child_margin_before_carrier_bytes": 42_914,
        "raw_base_lde_bytes": 20_199_768_064,
    }
    assert SCREEN["base_field_hiding"] == expected
    assert screen(502_895) == expected
    assert SCREEN["CAP"] == 524_288
    assert SCREEN["AXT_INNER_CAP"] == 1_048_576


def test_base_field_hiding_screen_refuses_drifted_frame_or_caps(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Source size and production-limit changes need an explicit review."""
    screen = SCREEN["base_field_hiding_degree_screen"]
    with pytest.raises(ValueError, match="hypothetical q64 reference frame changed"):
        screen(502_896)
    with monkeypatch.context() as patch:
        patch.setitem(screen.__globals__, "CAP", 524_289)
        with pytest.raises(ValueError, match="margins changed"):
            screen(502_895)
    with monkeypatch.context() as patch:
        patch.setitem(screen.__globals__, "AXT_INNER_CAP", 1_048_577)
        with pytest.raises(ValueError, match="margins changed"):
            screen(502_895)


def test_current_q77_source_bound_is_separate_from_hypothetical_models() -> None:
    """Neither q375 nor q64 screens may be labeled as current native geometry."""
    current = SCREEN["current_source"]
    assert current["current_geometry"]["queries"] == 77
    assert current["current_geometry"]["digest_bytes"] == 32
    assert current["offline_deep"] == {"max_frame_bytes": 500_084, "headroom": 24_204}
    assert current["production_qualified"] is False
    with pytest.raises(ValueError, match="hypothetical q64 reference frame changed"):
        SCREEN["base_field_hiding_degree_screen"](500_084)


def test_current_q77_linked_queries_attain_every_counted_frontier() -> None:
    """One linked query set simultaneously attains the current byte envelope."""
    parity_order = sorted(range(128), key=lambda value: (bin(value).count("1") % 2 == 0, value))
    positions = {(value | value << 7 | value << 14 | value << 21) & (8_388_608 - 1)
                 for value in parity_order[:77]}
    assert len(positions) == 77
    exact = SCREEN["exact_frontier"]
    upper = SCREEN["frontier"]
    domains = (8_388_608, 524_288, 32_768, 4_096, 512, 128)
    observed = []
    for leaves in domains:
        opened = {index % leaves for index in positions}
        assert len(opened) == 77
        assert exact(leaves, opened) == upper(leaves, 77)
        observed.append(exact(leaves, opened))
    assert observed == [1283, 975, 667, 436, 205, 51]
