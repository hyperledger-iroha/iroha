#!/usr/bin/env python3
"""Check exact arithmetic for the conditional compact FRI-only lemma."""

from fractions import Fraction
from hashlib import sha256
from math import comb
from pathlib import Path
import json
import re


def rational(value):
    """Render an exact rational as decimal numerator and denominator strings."""
    return {"numerator": str(value.numerator), "denominator": str(value.denominator)}


def check_source_geometry(root):
    """Reject stale numerical certificates when the selected live profile changes."""
    source = (root / "crates/fastpq_isi/src/params.rs").read_text()
    profile = source.split("pub const FASTPQ_FINAL_V1: StarkParameterSet =", 1)[1].split("\n};", 1)[0]
    for name, value in [("trace_log_size", 16), ("lde_log_size", 19), ("arity", 2), ("blowup_factor", 8), ("max_reductions", 17)]:
        assert re.search(rf"\b{name}:\s*{value}\s*,", profile), name
    assert re.search(r"pub const FASTPQ_QUERY_COUNT_V1: u32 = 136;", source)
    assert "field: GOLDILOCKS_FP4_V1," in profile
    assert "queries: FASTPQ_QUERY_COUNT_V1," in profile


def main():
    """Check the conditional geometry/bounds and retain a local exact result."""
    root = Path(__file__).resolve().parents[2]
    check_source_geometry(root)
    base = root / "target/fastpq-production-validation"
    base.mkdir(parents=True, exist_ok=True)
    p = 2**64 - 2**32 + 1
    size = 524288
    degree = 131072
    rounds = 17
    queries = 136
    gamma = Fraction(49, 100)
    agreement = 1 - gamma
    lengths = [size // 2**i for i in range(rounds + 1)]
    degrees = [degree // 2**i for i in range(rounds + 1)]
    assert lengths[-1] == 4 and degrees[-1] == 1
    assert all(n == 4 * d for n, d in zip(lengths, degrees))

    h = Fraction(101, 2)
    coefficient = 8 * (2 * h**5 + 3 * h * gamma * Fraction(1, 4))
    assert coefficient == Fraction(525505039897, 100)
    for n, d in zip(lengths[1:-1], degrees[1:-1]):
        rho = Fraction(d - 1, n)
        assert Fraction(1, 8) <= rho < Fraction(1, 4)
        # Strict square comparisons avoid numerical square roots.
        assert Fraction(1, 3)**2 < rho < Fraction(1, 2)**2
        # sqrt(rho)<1/2 gives eta>1/100 and sqrt(rho)/eta<50.
        assert 1 - Fraction(1, 2) - gamma == Fraction(1, 100)
        assert Fraction(1, 2) / Fraction(1, 100) == 50
        # 3*rho*sqrt(rho)>3*(1/8)*(1/3)=1/8.
        assert 3 * Fraction(1, 8) * Fraction(1, 3) == Fraction(1, 8)
        assert h * 3 < 152

    nonterminal_sum = sum(lengths[1:-1])
    assert nonterminal_sum == 524280 and len(lengths[1:-1]) == 16
    exceptions = coefficient * nonterminal_sum + 16 * 152 + comb(4, 2)
    assert exceptions == Fraction(13775589115872148, 5)
    commit_bound = exceptions / p**4
    assert commit_bound < Fraction(1, 2**204)

    threshold = agreement * size
    max_passing = (threshold.numerator + threshold.denominator - 1) // threshold.denominator - 1
    assert max_passing == 267386
    assert Fraction(max_passing, size) < agreement
    assert Fraction(max_passing + 1, size) >= agreement
    query_bound = Fraction(comb(max_passing, queries), comb(size, queries))
    assert query_bound <= agreement**queries
    total_bound = commit_bound + query_bound
    assert total_bound < Fraction(1, 2**132)

    paper = base / "soundness-paper-text" / "proximity2025.pdf"
    report = {
        "scope": "conditional interactive FRI only; not AIR, hash, Fiat-Shamir, qROM or production qualification",
        "date": "2026-09-06",
        "primary_theorem": "November 11, 2025 On Proximity Gaps for Reed-Solomon Codes, Theorem 4.2 and Corollary 4.4",
        "primary_pdf_sha256": "4added3e55b83c15fcc8a698fb57e137f5bd83e79ea25ce79382817c1ad26a46",
        "local_primary_pdf_matches": sha256(paper.read_bytes()).hexdigest() == "4added3e55b83c15fcc8a698fb57e137f5bd83e79ea25ce79382817c1ad26a46" if paper.exists() else None,
        "field_size": str(p**4),
        "initial_length": size,
        "exclusive_degree": degree,
        "folds": rounds,
        "terminal_length": 4,
        "terminal_exclusive_degree": 1,
        "distinct_initial_queries": queries,
        "strict_distance_threshold": rational(gamma),
        "nonterminal_exception_coefficient": rational(coefficient),
        "summed_exception_majorant": rational(exceptions),
        "commit_bound": rational(commit_bound),
        "commit_bound_strictly_below_power_of_two": -204,
        "max_good_transcript_passing_positions": max_passing,
        "query_bound": rational(query_bound),
        "total_bound_strictly_below_power_of_two": -132,
        "proof_review": "arithmetic and independent internal conditional-lemma review complete; no release security qualification",
    }
    output = base / "compact-fri-conditional-bound-certificate.json"
    output.write_text(json.dumps(report, indent=2) + "\n")
    print("PASS: exact conditional commit bound <2^-204 and commit+query bound <2^-132")
    print(output)


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Certificate checks require Python assertions; remove -O/PYTHONOPTIMIZE")
    main()
