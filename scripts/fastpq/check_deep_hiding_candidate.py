#!/usr/bin/env python3
"""Conditional DEEP hiding construction arithmetic; no prover or qualification.

Primary construction: Haböck–Al Kindi, ePrint 2024/1037, sections 3 and 4.1.
This file independently checks the finite opening map and the proposed extra
composition-mask field. It does not change or endorse the current wire format.
"""

from __future__ import annotations

from pathlib import Path
import re


P = 0xFFFFFFFF00000001
N = 65_536
M = 8_388_608
ROOT = 0x35C4528B4AA62EB8
OFFSET = 0xFD0E69F9A98EE946
QUERIES = 64
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


def candidate_screen(current_frame: int = 502_895) -> dict[str, int]:
    """Exact masked-composition DTO accounting, not a qualified private proof."""
    if current_frame != 502_895:
        raise ValueError("current DEEP frame changed; review candidate codec accounting")
    h = 2 * (4 + QUERIES)
    hp = 1 + QUERIES
    trace = N + h
    numerator = max(2 * trace - 1 + N - N // 512, trace + N - 1)
    quotient = numerator - N
    low, high = N + hp, max(quotient - N, hp)
    old_pair = field(4) + 2 * field(FP4_BYTES)
    proposed_triple = old_pair + field(FP4_BYTES)
    added_bytes = field(8 + QUERIES * field(proposed_triple)) - field(8 + QUERIES * field(old_pair))
    pre_mask_frame = 500_783
    candidate_bytes = pre_mask_frame + added_bytes
    if candidate_bytes != current_frame:
        raise ValueError("implemented composition-mask frame disagrees with nested codec charges")
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
        "implemented_candidate_frame_bytes": candidate_bytes,
        "candidate_margin_bytes": CAP - candidate_bytes,
        "candidate_two_child_margin_before_carrier": 1_048_576 - 2 * candidate_bytes,
        "witness_mask_payload_bytes": WIDTH * h * 8,
        "quotient_mask_payload_bytes": hp * FP4_BYTES,
        "composition_mask_coefficient_payload_bytes": 2 * N * FP4_BYTES,
        "trace_coefficients_masks_and_one_stripe_payload_bytes": 2 * WIDTH * N * 8 + WIDTH * h * 8,
        "one_materialized_full_row_lde_payload_bytes": WIDTH * M * 8,
        "one_materialized_binary_digest_tree_payload_bytes": (2 * M - 1) * 48,
        "one_full_fp4_oracle_payload_bytes": M * FP4_BYTES,
    }


def check_source_frame() -> None:
    """Reject silent drift from the implemented native codec's reviewed bound."""
    source = Path(__file__).resolve().parents[2] / "crates/fastpq_prover/src/backend/deep_proof.rs"
    matches = re.findall(r"MAX_FRAME_BYTES: usize = ([\d_]+);", source.read_text())
    if len(matches) != 1:
        raise ValueError("expected one implemented DEEP frame bound")
    candidate_screen(int(matches[0].replace("_", "")))


if __name__ == "__main__":
    check_source_frame()
    for key, value in candidate_screen().items():
        print(f"{key}={value}")
