#!/usr/bin/env python3
"""Exact arithmetic checks for the conditional compact FASTPQ sampler lemma.

Uses only the Python standard library. This is a mathematical certificate and
small exhaustive oracle check, not a Rust test or concrete-hash security test.
"""

from collections import Counter
from fractions import Fraction
from itertools import product
from math import comb, factorial
from pathlib import Path
import re


P = 2**64 - 2**32 + 1
MAX_COUNT = 512
MIN_DRAWS = 64
DRAWS_PER_INDEX = 8
LANES = 6


def check_source_constants() -> None:
    """Fail if the live sampler constants no longer match this certificate."""
    root = Path(__file__).resolve().parents[2]
    backend = (root / "crates/fastpq_prover/src/backend.rs").read_text()
    params = (root / "crates/fastpq_isi/src/params.rs").read_text()
    field = (root / "crates/fastpq_prover/src/field.rs").read_text()
    digest = (root / "crates/fastpq_isi/src/poseidon_digest384.rs").read_text()
    for text, name, expected in [
        (backend, "QUERY_MIN_DIGEST_DRAWS", MIN_DRAWS),
        (backend, "QUERY_DIGEST_DRAWS_PER_INDEX", DRAWS_PER_INDEX),
        (params, "FASTPQ_MAX_QUERY_COUNT_V1", MAX_COUNT),
        (field, "GOLDILOCKS_MODULUS_V1", P),
        (digest, "GOLDILOCKS_DIGEST384_LANES_V1", LANES),
    ]:
        match = re.search(rf"\b{name}\s*:\s*(?:u\d+|usize)\s*=\s*([0-9a-fA-F_x]+);", text)
        assert match, name
        assert int(match.group(1).replace("_", ""), 0) == expected, name


def draw_budget(count: int) -> int:
    """Return the supported digest budget for a nonempty desired sample."""
    assert 1 <= count <= MAX_COUNT
    return max(MIN_DRAWS, DRAWS_PER_INDEX * count)


def strict_dyadic_exponent(numerator: int, denominator: int) -> int:
    """Return b with an independently checked numerator/denominator < 2**(-b)."""
    assert 0 < numerator < denominator
    bits = max(0, denominator.bit_length() - numerator.bit_length() - 1)
    while numerator * 2 ** (bits + 1) < denominator:
        bits += 1
    while numerator * 2**bits >= denominator:
        bits -= 1
    assert numerator * 2**bits < denominator
    return bits


def uniform_all_domain_certificates() -> None:
    """Check the two analytic domain regimes with rational integer inequalities."""
    a0 = Fraction(2**53 - 1, 2**53)
    # For m <= D < 2m and m <= 512, the rejected remainder is <= 1022.
    assert 1022 * 2**53 < P
    series = sum((a0**j / factorial(j) for j in range(7)), Fraction(0))
    assert series > Fraction(125, 46)
    assert MAX_COUNT * 46**48 * 2**60 < 125**48
    print("all small domains: abort < 512*(46/125)^48 < 2^-60")

    weakest = (10**9, None)
    for count in range(1, MAX_COUNT + 1):
        candidates = LANES * draw_budget(count)
        failures = candidates - count + 1
        numerator = comb(candidates, count - 1) * 3**failures
        denominator = 4**failures
        assert numerator * 2**108 < denominator, count
        exponent = strict_dyadic_exponent(numerator, denominator)
        weakest = min(weakest, (exponent, count))
    assert weakest == (108, 8)
    print("all D >= 2m: abort < 2^-108; weakest certified case m=8")


def profile_certificate(domain: int, count: int) -> None:
    """Check a coarse dyadic version of the adaptive failure-position union bound."""
    assert 1 <= count <= min(domain, MAX_COUNT) and domain <= P
    quotient, remainder = divmod(P, domain)
    nonfresh_numerator = remainder + (count - 1) * quotient
    assert nonfresh_numerator > 0
    power = 0
    while nonfresh_numerator * 2 ** (power + 1) <= P:
        power += 1
    assert nonfresh_numerator * 2**power <= P
    candidates = LANES * draw_budget(count)
    failures = candidates - count + 1
    combinations = comb(candidates, count - 1)
    ceiling_log = combinations.bit_length()
    assert combinations < 2**ceiling_log
    exponent = power * failures - ceiling_log
    assert exponent > 0
    print(
        f"D={domain}, m={count}, draws={draw_budget(count)}, C={candidates}, "
        f"k={quotient}, r={remainder}: b<=2^-{power}; abort<2^-{exponent}"
    )


def dense_domain_lower_certificate() -> None:
    """Rule out incorrectly promising a universal 128-bit abort bound."""
    count = 512
    candidates = LANES * draw_budget(count)
    # Bonferroni: sum(single missing) minus sum(pair missing).
    # k/P <= 1/512 gives the first lower bound. Acceptance >= a0 and
    # the same rational-series bound give pair probability < (46/125)^96.
    first = Fraction(count * 511**candidates, 512**candidates)
    pair_upper = Fraction(comb(count, 2) * 46**96, 125**96)
    lower = first - pair_upper
    assert lower > Fraction(1, 2**61)
    print("D=m=512: 2^-61 < abort < 2^-60 (availability, not soundness error)")


def failure_numerator_dp(prime: int, domain: int, count: int, candidates: int) -> int:
    """Compute exact unfinished probability times prime**candidates by occupancy DP."""
    quotient, remainder = divmod(prime, domain)
    states = [1] + [0] * (count - 1)
    for _ in range(candidates):
        next_states = [0] * count
        for size, mass in enumerate(states):
            next_states[size] += mass * (remainder + quotient * size)
            if size + 1 < count:
                next_states[size + 1] += mass * quotient * (domain - size)
        states = next_states
    return sum(states)


def exhaustive_small_oracle_checks() -> None:
    """Compare literal six-lane enumeration with independent exact occupancy counts.

    Toy primes and deliberately tiny budgets make exhaustive checks practical;
    the analytic certificates above use the actual Goldilocks prime and caps.
    """
    for prime, domain, count, draws in [(3, 2, 2, 1), (5, 3, 2, 1), (5, 3, 3, 1), (3, 3, 2, 2)]:
        limit = prime - prime % domain
        completions = Counter()
        failed = 0
        candidates = LANES * draws
        for words in product(range(prime), repeat=candidates):
            order = []
            used_draws = None
            for offset in range(0, candidates, LANES):
                for word in words[offset : offset + LANES]:
                    if word < limit and word % domain not in order:
                        order.append(word % domain)
                    if len(order) == count:
                        used_draws = offset // LANES + 1
                        break
                if used_draws is not None:
                    break
            if used_draws is None:
                failed += 1
            else:
                completions[(tuple(order), used_draws)] += 1
        assert failed == failure_numerator_dp(prime, domain, count, candidates)
        assert failed + sum(completions.values()) == prime**candidates
        possible_orders = factorial(domain) // factorial(domain - count)
        for used in range(1, draws + 1):
            masses = [mass for (_, consumed), mass in completions.items() if consumed == used]
            if masses:
                assert len(masses) == possible_orders
                assert len(set(masses)) == 1
        print(f"exhaustive p={prime}, D={domain}, m={count}, B={draws}: exact symmetry/DP pass")

    # Uniform marginal lanes are insufficient: correlated rotations only yield
    # five of twenty ordered pairs (and five of ten subsets), despite uniform lanes.
    observed = {
        tuple(dict.fromkeys((start + i) % 5 for i in range(LANES)))[:2]
        for start in range(5)
    }
    assert len(observed) == 5 < factorial(5) // factorial(3)
    assert len({tuple(sorted(order)) for order in observed}) == 5 < comb(5, 2)
    print("correlated uniform-marginal counterexample: confirmed")


def main() -> None:
    """Run deterministic source checks and exact mathematical certificates."""
    check_source_constants()
    uniform_all_domain_certificates()
    profile_certificate(524_288, 136)
    profile_certificate(524_288, 512)
    profile_certificate(P, 512)
    dense_domain_lower_certificate()
    exhaustive_small_oracle_checks()
    print("PASS: exact integer/rational checks; no simulation or hash assumption test")


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Certificate checks require Python assertions; remove -O/PYTHONOPTIMIZE")
    main()
