#!/usr/bin/env python3
"""FASTPQ alternative-opening size screens; no proof or security qualification."""

from __future__ import annotations

import re
from functools import lru_cache
from fractions import Fraction
from math import prod
from pathlib import Path

CAP = 512 * 1024
AXT_INNER_CAP = 1024 * 1024


def field(length: int) -> int:
    assert length >= 0
    count = 1
    remaining = length
    while remaining >= 128:
        count += 1
        remaining >>= 7
    return count + length


def vector(count: int, element_length: int) -> int:
    assert count >= 0
    return 8 + count * field(element_length)


def record(*field_lengths: int) -> int:
    return sum(field(length) for length in field_lengths)


def parents(leaves: int, opened: int) -> int:
    assert leaves > 0 and leaves & (leaves - 1) == 0
    assert 1 <= opened <= leaves
    return sum(min(opened, 1 << level) for level in range(leaves.bit_length() - 1))


def frontier(leaves: int, opened: int) -> int:
    return parents(leaves, opened) - opened + 1


def exact_frontier(leaves: int, indices: set[int]) -> int:
    """Minimal binary Merkle frontier for one explicit set of leaf positions."""
    assert leaves > 0 and leaves & (leaves - 1) == 0
    assert indices and all(0 <= index < leaves for index in indices)
    opened = len(indices)
    level = indices
    parent_count = 0
    while leaves > 1:
        level = {index // 2 for index in level}
        parent_count += len(level)
        leaves //= 2
    return parent_count - opened + 1


def strided_fri_round_bytes(
    positions: tuple[int, ...], lde_rows: int, folded_power: int, power: int
) -> tuple[int, int, int]:
    """Return group values, Merkle frontier, and selected group count.

    This charges canonical 32-byte Fp4 values and 48-byte Merkle digests,
    omitting indices, framing, roots, rows, and all other proof material.
    """
    assert power > 0 and folded_power >= 0
    assert lde_rows > 0 and lde_rows & (lde_rows - 1) == 0
    leaves = lde_rows >> (folded_power + power)
    assert leaves > 0
    groups = {index % leaves for index in positions}
    return len(groups) * (1 << power) * 32, exact_frontier(leaves, groups) * 48, len(groups)


def min_strided_merkle_fri_bytes(
    positions: tuple[int, ...], lde_rows: int, terminal: int, max_fold_power: int
) -> tuple[int, tuple[int, ...]]:
    """Minimize FRI value plus 48-byte frontier costs over fold partitions.

    This is only a byte screen for the current strided-group Merkle-opening
    model. It makes no claim that another fold schedule is sound or implemented.
    """
    assert len(positions) == len(set(positions))
    assert positions and all(0 <= index < lde_rows for index in positions)
    assert terminal > 0 and terminal & (terminal - 1) == 0
    ratio = lde_rows // terminal
    assert terminal * ratio == lde_rows and ratio & (ratio - 1) == 0
    total_power = ratio.bit_length() - 1
    assert 1 <= max_fold_power <= total_power

    @lru_cache(maxsize=None)
    def best(folded_power: int) -> tuple[int, tuple[int, ...]]:
        if folded_power == total_power:
            return 0, ()
        optimum = (10**30, ())
        for power in range(1, min(max_fold_power, total_power - folded_power) + 1):
            values, siblings, _ = strided_fri_round_bytes(
                positions, lde_rows, folded_power, power
            )
            remaining, schedule = best(folded_power + power)
            candidate = (values + siblings + remaining, (power,) + schedule)
            if candidate[0] < optimum[0]:
                optimum = candidate
        return optimum

    return best(0)


def exhaustive_strided_merkle_fri_bytes(
    positions: tuple[int, ...], lde_rows: int, terminal: int, max_fold_power: int
) -> tuple[int, tuple[int, ...], int, int]:
    """Independently enumerate all fold-bit boundary masks for a byte certificate.

    At 17 fold bits there are exactly 2^16 ordered power-of-two partitions.
    This exhaustive check corroborates the dynamic-programming optimizer,
    but neither search is a FRI soundness or production-cost proof.
    """
    assert len(positions) == len(set(positions))
    assert positions and all(0 <= index < lde_rows for index in positions)
    assert terminal > 0 and terminal & (terminal - 1) == 0
    ratio = lde_rows // terminal
    assert terminal * ratio == lde_rows and ratio & (ratio - 1) == 0
    total_power = ratio.bit_length() - 1
    assert 1 <= max_fold_power <= total_power
    transitions = {
        (start, power): sum(strided_fri_round_bytes(positions, lde_rows, start, power)[:2])
        for start in range(total_power)
        for power in range(1, min(max_fold_power, total_power - start) + 1)
    }
    best = (10**30, ())
    winners = 0
    admissible = 0
    for mask in range(1 << (total_power - 1)):
        start = 0
        cost = 0
        schedule = []
        for boundary in range(1, total_power):
            if mask & (1 << (boundary - 1)):
                power = boundary - start
                if power > max_fold_power:
                    break
                cost += transitions[start, power]
                schedule.append(power)
                start = boundary
        else:
            power = total_power - start
            if power > max_fold_power:
                continue
            cost += transitions[start, power]
            schedule.append(power)
            admissible += 1
            candidate = (cost, tuple(schedule))
            if cost < best[0]:
                best = candidate
                winners = 1
            elif cost == best[0]:
                winners += 1
                if candidate < best:
                    best = candidate
    assert admissible > 0
    return best[0], best[1], winners, admissible


def hypothetical_frame(
    query_count: int,
    row_width: int,
    lde_rows: int,
    terminal: int,
    arities: tuple[int, ...],
) -> dict[str, int]:
    """Fixed-width row DTO generalized to hypothetical geometry/frontiers."""
    assert prod(arities) == lde_rows // terminal
    assert lde_rows & (lde_rows - 1) == 0
    rows = min(2 * query_count, lde_rows)
    # RowValues is one fixed-size canonical little-endian byte field. The
    # earlier variable-row vector overcounted 350 bytes per current row.
    row_element = record(4, row_width * 8)
    query_element = record(4, 32, 32)
    round_payload = 8
    fri_values = 0
    fri_siblings = 0
    fri_leaves = 0
    fri_parents = 0
    length = lde_rows
    for arity in arities:
        leaves = length // arity
        groups = min(query_count, leaves)
        siblings = frontier(leaves, groups)
        group_element = record(4, record(*([32] * arity)))
        round_payload += field(record(vector(groups, group_element), vector(siblings, 48)))
        fri_values += groups * arity * 32
        fri_siblings += siblings
        fri_leaves += groups
        fri_parents += parents(leaves, groups)
        length = leaves
    assert length == terminal
    row_siblings = frontier(lde_rows, rows)
    scalar_siblings = frontier(lde_rows, query_count)
    proof_bytes = 40 + record(
        48,
        48,
        48,
        vector(len(arities) + 1, 48),
        vector(rows, row_element),
        vector(query_count, query_element),
        vector(row_siblings, 48),
        vector(scalar_siblings, 48),
        vector(scalar_siblings, 48),
        round_payload,
        vector(terminal, 32),
    )
    membership_hashes = (
        rows + 2 * query_count + fri_leaves + 1
        + parents(lde_rows, rows) + 2 * parents(lde_rows, query_count)
        + fri_parents + 1
    )
    return {
        "frame_bytes": proof_bytes,
        "fri_value_bytes": fri_values,
        "fri_sibling_bytes": 48 * fri_siblings,
        "membership_hashes": membership_hashes,
    }


def miss_probability(good: int, domain: int, queries: int) -> Fraction:
    """Exact subset-miss term from the current conditional AIR reduction."""
    assert 0 <= queries <= good <= domain
    numerator = prod(good - i for i in range(queries))
    denominator = prod(domain - i for i in range(queries))
    return Fraction(numerator, denominator)


def base_field_hiding_degree_screen(deep_frame_bytes: int) -> dict[str, int]:
    """Unqualified `<2N` DEEP/masking byte and degree screen, not a proof."""
    n, h, lde_rows, queries, retained = 65_536, 32_768, 8_388_608, 64, 301
    if deep_frame_bytes != 500_783:
        raise ValueError("DEEP DTO frame changed; review the hiding candidate")
    trace_bound = n + h
    numerator_bound = max(2 * trace_bound - 1 + (n - n // 512), trace_bound + n - 1)
    quotient_bound = numerator_bound - n
    high_quotient_bound = quotient_bound - n
    # Candidate appends R(x) to the same fixed inline RowValues field, with
    # no per-cell framing or new Merkle tree. Both lengths use two-byte varints.
    row_mask_bytes = field(retained * 8 + 32) - field(retained * 8)
    frame_bytes = deep_frame_bytes + queries * row_mask_bytes
    segment_margin = CAP - frame_bytes
    axt_two_child_margin = AXT_INNER_CAP - 2 * frame_bytes
    if (
        (trace_bound, numerator_bound, quotient_bound, high_quotient_bound)
        != (98_304, 262_015, 196_479, 130_943)
        or trace_bound >= 2 * n
        or high_quotient_bound >= 2 * n
        or row_mask_bytes != 32
        or queries * row_mask_bytes != 2_048
        or (frame_bytes, segment_margin, axt_two_child_margin)
        != (502_831, 21_457, 42_914)
    ):
        raise ValueError("unqualified hiding degree or byte margins changed")
    return {
        "trace_degree_bound": trace_bound,
        "numerator_degree_bound": numerator_bound,
        "quotient_degree_bound": quotient_bound,
        "high_quotient_degree_bound": high_quotient_bound,
        "terminal_degree_bound": 2,
        "row_mask_added_bytes": queries * row_mask_bytes,
        "frame_bytes": frame_bytes,
        "segment_margin_bytes": segment_margin,
        "axt_two_child_margin_before_carrier_bytes": axt_two_child_margin,
        "raw_base_lde_bytes": retained * lde_rows * 8,
    }


deep_source = (
    Path(__file__).resolve().parents[2]
    / "crates/fastpq_prover/src/backend/deep_proof.rs"
).read_text()
deep_matches = re.findall(r"MAX_FRAME_BYTES: usize = ([\d_]+);", deep_source)
if len(deep_matches) != 1:
    raise ValueError("expected one DEEP DTO frame bound in source")
base_field_hiding = base_field_hiding_degree_screen(
    int(deep_matches[0].replace("_", ""))
)


current = hypothetical_frame(375, 342, 524288, 4, (2,) * 17)
vertical64 = hypothetical_frame(64, 32, 8388608, 4, (8,) * 7)
vertical72 = hypothetical_frame(72, 32, 8388608, 4, (8,) * 7)
assert current["frame_bytes"] == 4_017_376
assert vertical64["frame_bytes"] == 469_093
assert vertical72["frame_bytes"] == 519_326

# A transcript can select this exact 375-element subset: its first 375
# canonical query coordinates can encode the distinct indices directly.
# For this one valid subset, even the cheapest power-of-two fold schedule
# using the current full-group values and Merkle frontiers is over 512 KiB.
spread_queries = tuple(index * 524_288 // 375 for index in range(375))
assert len(set(spread_queries)) == 375
arity_min, arity_schedule = min_strided_merkle_fri_bytes(
    spread_queries, 524_288, 4, 17
)
assert (arity_min, arity_schedule) == (556_496, (3, 3, 3, 8))
assert arity_min - CAP == 32_208
assert min_strided_merkle_fri_bytes(spread_queries, 524_288, 4, 3)[0] == 561_104
implemented_arity_min, implemented_arity_schedule = min_strided_merkle_fri_bytes(
    spread_queries, 524_288, 4, 4
)
assert (implemented_arity_min, implemented_arity_schedule) == (558_544, (3, 3, 3, 4, 4))
all_partitions = exhaustive_strided_merkle_fri_bytes(spread_queries, 524_288, 4, 17)
implemented_partitions = exhaustive_strided_merkle_fri_bytes(spread_queries, 524_288, 4, 4)
assert all_partitions[:2] == (arity_min, arity_schedule)
assert implemented_partitions[:2] == (implemented_arity_min, implemented_arity_schedule)

miss64 = miss_probability(360447, 524288, 64)
miss72 = miss_probability(360447, 524288, 72)
assert Fraction(1, 2**35) < miss64 < Fraction(1, 2**34)
assert Fraction(1, 2**39) < miss72 < Fraction(1, 2**38)

print("current fixed-row q375/w342, binary, maximal-frame model:", current)
print("screened q64/w32/N=2^20/L=2^23, seven arity-8 groups:", vertical64)
print("screened q72/w32/N=2^20/L=2^23, seven arity-8 groups:", vertical72)
print("q64 per-segment headroom:", CAP - vertical64["frame_bytes"])
print(
    "q64 two-segment inner AXT headroom before carrier:",
    AXT_INNER_CAP - 2 * vertical64["frame_bytes"],
)
print("q72 per-segment headroom:", CAP - vertical72["frame_bytes"])
print(
    "q72 two-segment inner AXT headroom before carrier:",
    AXT_INNER_CAP - 2 * vertical72["frame_bytes"],
)
print("q375 strided Merkle FRI minimum for one valid subset:", arity_min, arity_schedule)
print("q375 exhaustive fold partitions (all powers):", all_partitions)
print("q375 exhaustive fold partitions (arity at most 16):", implemented_partitions)
print("q64 conditional blowup-8 query term strictly between 2^-35 and 2^-34")
print("q72 conditional blowup-8 query term strictly between 2^-39 and 2^-38")
print("q64 hypothetical raw LDE u64 matrix:", 8388608 * 32 * 8)
print("current raw LDE u64 matrix:", 524288 * 342 * 8)
print("unqualified base-field-hiding <2N DEEP candidate screen:", base_field_hiding)
print("qualification: false")
