#!/usr/bin/env python3
"""Conditional DEEP hiding construction arithmetic; no prover or qualification.

Primary construction: Haböck–Al Kindi, ePrint 2024/1037, sections 3 and 4.1.
This file checks finite opening maps and the current q77 composition-mask field,
and preserves a separately labeled q64 counterfactual. Neither screen endorses
the protocol or supplies a soundness or zero-knowledge proof.
"""

from __future__ import annotations

from pathlib import Path
from runpy import run_path


P = 0xFFFFFFFF00000001
N = 65_536
M = 8_388_608
ROOT = 0x35C4528B4AA62EB8
OFFSET = 0xFD0E69F9A98EE946
QUERIES = 77
WIDTH = 301
FP4_BYTES = 32
CAP = 524_288


def fp4_mul(a: tuple[int, ...], b: tuple[int, ...]) -> tuple[int, ...]:
    """Reference multiplication in the existing Fp[u]/(u^4-7) basis."""
    result = [0] * 4
    for i, ai in enumerate(a):
        for j, bj in enumerate(b):
            degree = i + j
            result[degree % 4] += ai * bj * (7 if degree >= 4 else 1)
    return tuple(value % P for value in result)


def fp4_pow(a: tuple[int, ...], exponent: int) -> tuple[int, ...]:
    """Reference square-and-multiply with an explicit public exponent."""
    result = (1, 0, 0, 0)
    while exponent:
        if exponent & 1:
            result = fp4_mul(result, a)
        a = fp4_mul(a, a)
        exponent >>= 1
    return result


def opening_matrix(indices: list[int], mask_coefficients: int,
                   z: tuple[int, ...], include_next: bool = True) -> list[list[int]]:
    """Base-linear mask map for QD (plus g QD) and both OOD answers.

    Rows include the actual nonzero X^N-1 multiplier, including its full
    extension-field action at z and gz. The production challenge may lie in
    the quadratic subfield, so the two OOD points do not always have rank eight.
    """
    if (len(indices) != len(set(indices)) or not indices
            or any(not 0 <= i < M for i in indices) or mask_coefficients < 1):
        raise ValueError("invalid explicit opening geometry")
    g = pow(ROOT, M // N, P)
    closure = set(indices)
    if include_next:
        closure.update((i + M // N) % M for i in indices)
    matrix = []
    for index in sorted(closure):
        x = OFFSET * pow(ROOT, index, P) % P
        vanishing = (pow(x, N, P) - 1) % P
        assert vanishing != 0
        row, value = [], vanishing
        for _ in range(mask_coefficients):
            row.append(value)
            value = value * x % P
        matrix.append(row)
    for point in [z, tuple(g * v % P for v in z)]:
        power = fp4_pow(point, N)
        value = ((power[0] - 1) % P, *power[1:])
        assert value != (0, 0, 0, 0)
        rows: list[list[int]] = [[], [], [], []]
        for _ in range(mask_coefficients):
            for coordinate, row in enumerate(rows):
                row.append(value[coordinate])
            value = fp4_mul(value, point)
        matrix.extend(rows)
    return matrix


def base_rank(matrix: list[list[int]]) -> int:
    """Exact Gaussian rank over Goldilocks; never a numerical approximation."""
    rows = [list(row) for row in matrix]
    if not rows or not rows[0] or any(len(row) != len(rows[0]) for row in rows):
        raise ValueError("rank needs a rectangular nonempty matrix")
    rank = 0
    for column in range(len(rows[0])):
        pivot = next((i for i in range(rank, len(rows)) if rows[i][column] % P), None)
        if pivot is None:
            continue
        rows[rank], rows[pivot] = rows[pivot], rows[rank]
        inverse = pow(rows[rank][column], P - 2, P)
        rows[rank] = [value * inverse % P for value in rows[rank]]
        for i in range(rank + 1, len(rows)):
            factor = rows[i][column] % P
            if factor:
                rows[i] = [(a - factor * b) % P for a, b in zip(rows[i], rows[rank])]
        rank += 1
        if rank == len(rows):
            break
    return rank


def field(payload: int) -> int:
    """Current Norito compact field-length prefix plus exact payload."""
    if payload < 0:
        raise ValueError("negative payload")
    return payload + max(1, (payload.bit_length() + 6) // 7)


def _mask_screen(queries: int, expected_frame: int, pre_mask_frame: int,
                 digest_bytes: int) -> dict[str, int]:
    """Exact conditional equations and nested codec charges for explicit inputs."""
    h = 2 * (4 + queries)
    hp = 1 + queries
    trace = N + h
    numerator = max(2 * trace - 1 + N - N // 512, trace + N - 1)
    quotient = numerator - N
    low, high = N + hp, max(quotient - N, hp)
    old_pair = field(4) + 2 * field(FP4_BYTES)
    proposed_triple = old_pair + field(FP4_BYTES)
    added_bytes = field(8 + queries * field(proposed_triple)) - field(8 + queries * field(old_pair))
    candidate_bytes = pre_mask_frame + added_bytes
    if candidate_bytes != expected_frame:
        raise ValueError("modeled composition-mask frame disagrees with nested codec charges")
    return {
        "witness_mask_base_coefficients": h,
        "quotient_mask_fp4_coefficients": hp,
        "trace_degree_bound": trace,
        "numerator_degree_bound": numerator,
        "quotient_degree_bound": quotient,
        "randomized_low_degree_bound": low,
        "randomized_high_degree_bound": high,
        "pre_mask_candidate_frame_bytes": pre_mask_frame,
        "extra_framed_composition_mask_bytes": added_bytes,
        "modeled_candidate_frame_bytes": candidate_bytes,
        "candidate_margin_bytes": CAP - candidate_bytes,
        "candidate_two_child_margin_before_carrier": 1_048_576 - 2 * candidate_bytes,
        "witness_mask_payload_bytes": WIDTH * h * 8,
        "quotient_mask_payload_bytes": hp * FP4_BYTES,
        "composition_mask_coefficient_payload_bytes": 2 * N * FP4_BYTES,
        "trace_coefficients_masks_and_one_stripe_payload_bytes": 2 * WIDTH * N * 8 + WIDTH * h * 8,
        "one_materialized_full_row_lde_payload_bytes": WIDTH * M * 8,
        "one_materialized_binary_digest_tree_payload_bytes": (2 * M - 1) * digest_bytes,
        "one_full_fp4_oracle_payload_bytes": M * FP4_BYTES,
    }


def hypothetical_q64_full_fiber_screen() -> dict[str, int]:
    """Preserved q64/full-fiber/48-byte-digest counterfactual, not current source."""
    return _mask_screen(64, 502_895, 500_783, 48)


def candidate_screen(current_frame: int = 500_084) -> dict[str, int]:
    """Exact current q77 masked-composition accounting, not private-proof qualification."""
    if current_frame != 500_084:
        raise ValueError("current DEEP frame changed; review candidate codec accounting")
    source_budget = run_path(str(Path(__file__).with_name("check_compact_source_budget.py")))
    sources = source_budget["load_sources"]()
    geometry = source_budget["current_geometry"](sources)
    if (geometry["queries"], geometry["retained_columns"], geometry["trace_rows"], geometry["lde_rows"]) != (QUERIES, WIDTH, N, M):
        raise ValueError("opening-map constants differ from current source geometry")
    # The same DTO with only its composition-mask field removed is a byte
    # counterfactual; there is no corresponding supported decoder.
    pre_mask_frame = source_budget["deep_frame_bound"](sources, composition_mask=False)
    return _mask_screen(QUERIES, current_frame, pre_mask_frame, geometry["digest_bytes"])


def check_source_frame() -> None:
    """Reject silent drift from the implemented native codec's reviewed bound."""
    source_budget = run_path(str(Path(__file__).with_name("check_compact_source_budget.py")))
    result = source_budget["budget"](source_budget["load_sources"]())
    candidate_screen(result["offline_deep"]["max_frame_bytes"])


if __name__ == "__main__":
    check_source_frame()
    for key, value in candidate_screen().items():
        print(f"{key}={value}")
