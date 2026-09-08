#!/usr/bin/env python3
"""Exact conditional SHAKE bundle accounting; no production qualification."""

from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from math import isqrt
from pathlib import Path

import check_compact_typed_profile as profile


def calculate(segments: int) -> dict:
    """Charge every segment under one adaptive-context family and query budget."""
    if type(segments) is not int or not 1 <= segments <= profile.MAX_BUNDLE_SEGMENTS:
        raise ValueError("segments must be an integer in 1..=128")
    work = profile.counters(375)
    candidates, tapes, field_abort, query_abort = profile.tapes(375)
    assert candidates == 401
    total_queries = 2 * (profile.ADVERSARY + segments * work["verifier_calls"])
    delta = max(
        [Fraction(3 * (total_queries - 1), profile.P**6)]
        + [
            error + Fraction(total_queries - 1, 2**bits)
            for error, bits in zip(profile.errors(375), tapes)
        ]
    )
    a = 6 * total_queries**2 * delta
    b = 2 * segments * (
        Fraction(work["H_calls"], profile.P**6)
        + sum(Fraction(1, 2**bits) for bits in tapes)
    )
    z = profile.TARGET / profile.TARGETS - a - b
    product = a * b
    floor = isqrt((product.numerator << 768) // product.denominator)
    lower_root = Fraction(floor, 2**384)
    upper_root = lower_root + Fraction(1, 2**384)
    assert lower_root**2 <= product < upper_root**2
    lower = profile.TARGETS * (a + b + 2 * lower_root) / profile.TARGET
    upper = profile.TARGETS * (a + b + 2 * upper_root) / profile.TARGET
    abort = profile.TARGETS * segments * (field_abort + query_abort)
    return {
        "segments": segments,
        "H_calls": segments * work["H_calls"],
        "G_calls": segments * work["G_calls"],
        "group_query_budget": total_queries,
        "passes_conditional_54_target_acceptance_bound": z > 0 and z**2 > 4 * a * b,
        "scaled_acceptance_interval_over_1024": [
            lower.numerator * 1024 // lower.denominator,
            (upper.numerator * 1024 + upper.denominator - 1) // upper.denominator,
        ],
        "query_candidates": candidates,
        "query_tape_bytes": tapes[-1] // 8,
        "total_honest_attempts": profile.TARGETS * segments,
        "honest_abort_below_2_to_minus": profile.certified_bits(abort),
        "honest_abort_below_2_to_minus_128": abort < profile.TARGET,
    }


def controls() -> None:
    """Pin one-segment parity, bundle counts and separate honest-abort behavior."""
    single = calculate(1)
    reference = profile.aggregate_parts(375)
    interval = reference["aggregate_times_2_to_128_interval"]
    assert single["group_query_budget"] == reference["group_query_budget"]
    assert single["scaled_acceptance_interval_over_1024"] == [
        interval["lower_numerator"], interval["upper_numerator"]
    ]
    pair = calculate(2)
    assert pair["H_calls"] == 89124 and pair["G_calls"] == 44
    assert pair["group_query_budget"] == 8590112928
    assert pair["passes_conditional_54_target_acceptance_bound"]
    assert pair["honest_abort_below_2_to_minus"] == 136
    largest = calculate(128)
    assert largest["passes_conditional_54_target_acceptance_bound"]
    assert largest["scaled_acceptance_interval_over_1024"] == [745, 746]
    assert largest["honest_abort_below_2_to_minus"] == 130
    assert largest["honest_abort_below_2_to_minus_128"]
    for segments in range(1, profile.MAX_BUNDLE_SEGMENTS + 1):
        result = calculate(segments)
        assert result["honest_abort_below_2_to_minus_128"]
        assert result["passes_conditional_54_target_acceptance_bound"]
    for invalid in [0, -1, 129, True, 1.5]:
        try:
            calculate(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError(f"invalid segment count accepted: {invalid}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--segments", type=int, nargs="+", default=[1, 2, 128])
    parser.add_argument(
        "--output", type=Path,
        default=profile.ROOT / "target/fastpq-production-validation/compact-bundle-profile-certificate.json",
    )
    args = parser.parse_args()
    if any(not 1 <= count <= 128 for count in args.segments):
        parser.error("each segment count must be in 1..=128")
    contracts = profile.check_source_contracts()
    controls()
    sources = [
        Path(__file__), Path(profile.__file__),
        profile.ROOT / "specs/fastpq_compact_typed_profile.md",
        profile.ROOT / "specs/fastpq_compact_adaptive_context.md",
        profile.ROOT / "crates/fastpq_prover/src/backend/compact_shake_candidate.rs",
        profile.ROOT / "crates/fastpq_prover/src/backend/compact_protocol/profile.rs",
    ]
    report = {
        "status": "pass",
        "qualification": False,
        "honest_attempt_envelope": profile.HONEST_ATTEMPTS,
        "query_candidates": 401,
        "query_tape_bytes": 953,
        "assumptions": [
            "fixed ideal SHAKE candidate profile and adaptive-context family extraction",
            "a false accepted bundle contains a false child in the admissible family",
            "one shared adversary budget of 2^32 binary queries and 54 external targets",
            "all S child expansions charged; no additional outer H/G oracle calls",
            "native public-state hashes, authority authentication and concrete primitive errors remain separate",
        ],
        "source_contracts": contracts,
        "source_sha256": {str(p.relative_to(profile.ROOT)): sha256(p.read_bytes()).hexdigest() for p in sources},
        "reports": [calculate(count) for count in args.segments],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": "pass", "qualification": False,
        "honest_attempt_envelope": profile.HONEST_ATTEMPTS,
        "query_candidates": 401,
        "query_tape_bytes": 953, "reports": report["reports"], "output": str(args.output)}, indent=2))


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Exact certificate requires assertions; remove -O.")
    main()
