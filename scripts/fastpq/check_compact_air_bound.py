#!/usr/bin/env python3
"""Exact arithmetic for the conditional compact formal-AIR IOP reduction."""

from fractions import Fraction as F
from hashlib import sha256
from math import comb
from pathlib import Path
import json

from check_compact_fri_bound import check_source_geometry


def rational(value):
    """Render an exact rational without converting to floating point."""
    return {"numerator": str(value.numerator), "denominator": str(value.denominator)}


def main():
    """Check the conditional bound against the current prototype geometry."""
    root = Path(__file__).resolve().parents[2]
    check_source_geometry(root)
    base = root / "target/fastpq-production-validation"
    base.mkdir(parents=True, exist_ok=True)
    p = 2**64 - 2**32 + 1
    n = 65536
    length = 8 * n
    width = 342
    agreement = F(11, 16)
    gamma = 1 - agreement
    h = F(7, 2)
    assert (2 * agreement - 1) * length == 3 * n
    assert 3 * n > 3 * n - 1
    assert agreement * length >= 3 * n
    assert (2 * agreement - 1) * length > 2 * n - 1
    assert (2 * agreement - 1) * length > n - 1

    c_fri = 8 * (2 * h**5 + 3 * h * gamma * F(1, 4))
    c_row = F(64, 3) * (2 * h**5 + 3 * h * gamma * F(1, 8))
    assert c_fri == F(134561, 16)
    assert c_row == F(269017, 12)
    fri_lengths = [length // 2**i for i in range(1, 17)]
    for size in [length] + fri_lengths:
        rho = F(size // 4 - 1, size)
        assert F(1, 8) <= rho < F(1, 4)
        assert F(1, 3)**2 < rho < F(1, 2)**2
        assert F(1, 2) / (1 - F(1, 2) - gamma) < 3
        assert 3 * F(1, 8) * F(1, 3) == F(1, 8)
        assert 3 * h < 11
    rho_row = F(n - 1, length)
    assert F(1, 16) < rho_row < F(1, 8)
    assert F(1, 4)**2 < rho_row < F(3, 8)**2
    assert F(3, 8) / (1 - F(3, 8) - gamma) < 3
    assert 3 * F(1, 16) * F(1, 4) == F(3, 64)
    assert 4 * h == 14

    b_row = c_row * length + 14
    b_joint = c_fri * length + 11
    assert sum(fri_lengths) == 524280
    b_fri = c_fri * sum(fri_lengths) + 16 * 11 + comb(4, 2)
    assert b_row == F(35260596266, 3)
    assert b_joint == 4409294859
    assert b_fri == F(8818455499, 2)
    exceptions = width * b_row + length + 1 + 2 * b_joint + b_fri
    assert exceptions == F(8065872632161, 2)
    commit_bound = exceptions / p**4
    assert commit_bound < F(1, 2**214)

    assert (agreement * length).denominator == 1
    max_passing = int(agreement * length) - 1
    assert max_passing == 360447
    results = []
    for queries in (136, 200, 236, 237):
        query_bound = F(comb(max_passing, queries), comb(length, queries))
        total = commit_bound + query_bound
        passes = total < F(1, 2**128)
        assert passes == (queries == 237)
        results.append({
            "distinct_queries": queries,
            "query_bound": rational(query_bound),
            "total_bound_strictly_below_2_to_minus_128": passes,
        })
    output = base / "compact-air-conditional-bound-certificate.json"
    primary_pdf = base / "soundness-paper-text/proximity2025.pdf"
    primary_sha = "4added3e55b83c15fcc8a698fb57e137f5bd83e79ea25ce79382817c1ad26a46"
    if primary_pdf.exists():
        assert sha256(primary_pdf.read_bytes()).hexdigest() == primary_sha
    report = {
        "scope": "conditional formal-AIR interactive soundness; not semantic AIR, concrete hash, Fiat-Shamir, qROM or production qualification",
        "date": "2026-09-06",
        "primary_pdf_sha256": primary_sha,
        "retained_primary_pdf_present_and_verified": primary_pdf.exists(),
        "trace_length": n,
        "evaluation_length": length,
        "width": width,
        "agreement_threshold": rational(agreement),
        "row_exception_majorant": rational(b_row),
        "joint_exception_majorant": rational(b_joint),
        "fri_exception_majorant": rational(b_fri),
        "summed_exception_majorant": rational(exceptions),
        "commit_bound": rational(commit_bound),
        "commit_bound_strictly_below_power_of_two": -214,
        "max_good_transcript_passing_positions": max_passing,
        "query_comparisons": results,
        "proof_review": "exact arithmetic checked; separate internal derivation review completed; external qualification remains required",
    }
    output.write_text(json.dumps(report, indent=2) + "\n")
    print("PASS: exact commit bound <2^-214; 237-query total <2^-128; 236-query bound does not reach target")
    print(output)


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Certificate checks require Python assertions; remove -O/PYTHONOPTIMIZE")
    main()
