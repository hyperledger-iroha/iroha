#!/usr/bin/env python3
"""Certify the compact prototype's prime and extension with exact arithmetic.

This independent polynomial implementation checks a Lucas primality certificate
and the degree-four Rabin criterion. It does not call the Rust field arithmetic
or establish any hash, protocol, or production-security claim.
"""

from hashlib import sha256
from math import gcd, isqrt
from pathlib import Path
import json
import re


def trim(poly, prime):
    """Return canonical ascending coefficients, using [] for zero."""
    result = [value % prime for value in poly]
    while result and result[-1] == 0:
        result.pop()
    return result


def remainder(poly, divisor, prime):
    """Perform ordinary polynomial long division over the prime field."""
    result = trim(poly, prime)
    divisor = trim(divisor, prime)
    if not divisor:
        raise ValueError("zero polynomial divisor")
    inverse = pow(divisor[-1], -1, prime)
    while len(result) >= len(divisor):
        shift = len(result) - len(divisor)
        factor = result[-1] * inverse % prime
        for index, coefficient in enumerate(divisor):
            result[index + shift] -= factor * coefficient
        result = trim(result, prime)
    return result


def multiply_mod(left, right, modulus, prime):
    """Convolve ordinary polynomials, then reduce by long division."""
    if not left or not right:
        return []
    result = [0] * (len(left) + len(right) - 1)
    for i, a in enumerate(left):
        for j, b in enumerate(right):
            result[i + j] += a * b
    return remainder(result, modulus, prime)


def power_mod(base, exponent, modulus, prime):
    """Exponentiate by repeated squaring in the polynomial quotient ring."""
    if exponent < 0:
        raise ValueError("negative exponent")
    result = [1]
    while exponent:
        if exponent & 1:
            result = multiply_mod(result, base, modulus, prime)
        base = multiply_mod(base, base, modulus, prime)
        exponent //= 2
    return result


def polynomial_gcd(left, right, prime):
    """Compute the monic greatest common divisor by Euclid's algorithm."""
    left, right = trim(left, prime), trim(right, prime)
    while right:
        left, right = right, remainder(left, right, prime)
    if not left:
        return []
    inverse = pow(left[-1], -1, prime)
    return [(value * inverse) % prime for value in left]


def subtract_x(poly, prime):
    """Subtract the indeterminate from a polynomial in ascending order."""
    result = list(poly) + [0] * max(0, 2 - len(poly))
    result[1] -= 1
    return trim(result, prime)


def quartic_certificate(modulus, prime):
    """Return the exact degree-four Rabin identities; prime is a premise."""
    modulus = trim(modulus, prime)
    if len(modulus) != 5:
        raise ValueError("expected degree four")
    x = [0, 1]
    p_squared = power_mod(x, prime**2, modulus, prime)
    p_fourth = power_mod(p_squared, prime**2, modulus, prime)
    common = polynomial_gcd(modulus, subtract_x(p_squared, prime), prime)
    return common == [1] and p_fourth == x, common, p_fourth


def main():
    """Verify the live constants, independent controls, and exact certificates."""
    root = Path(__file__).resolve().parents[2]
    field_path = root / "crates/fastpq_prover/src/field.rs"
    params_path = root / "crates/fastpq_isi/src/params.rs"
    field, params = field_path.read_text(), params_path.read_text()
    prime = 2**64 - 2**32 + 1
    for name, expected in [("GOLDILOCKS_MODULUS_V1", prime), ("FP4_NON_RESIDUE_V1", 7)]:
        match = re.search(rf"\b{name}: u64 = ([0-9a-fA-F_x]+);", field)
        assert match and int(match.group(1).replace("_", ""), 0) == expected
    assert 'extension_polynomial: "X^4 - 7",' in params

    # Complete factorization of p-1. Trial division proves each small factor
    # prime; the order certificate below then proves p itself prime.
    factors = {2: 32, 3: 1, 5: 1, 17: 1, 257: 1, 65537: 1}
    product = 1
    for factor, multiplicity in factors.items():
        assert all(factor % candidate for candidate in range(2, isqrt(factor) + 1))
        product *= factor**multiplicity
    assert product == prime - 1
    generator = 7
    assert pow(generator, prime - 1, prime) == 1
    residues = {factor: pow(generator, (prime - 1) // factor, prime) for factor in factors}
    assert all(gcd(value - 1, prime) == 1 for value in residues.values())

    # Small independent positive/negative controls for the polynomial checker.
    assert remainder([1, 0, 1], [1, 1], 5) == [2]
    assert polynomial_gcd([4, 0, 1], [4, 1], 5) == [4, 1]
    assert quartic_certificate([-2, 0, 0, 0, 1], 5)[0]
    for polynomial in [[-1, 0, 0, 0, 1], [0, 0, 0, 0, 1], [1, 0, 2, 0, 1]]:
        assert not quartic_certificate(polynomial, 5)[0]
    assert not quartic_certificate([-7, 0, 0, 0, 1], 3)[0]

    irreducible, common, fourth = quartic_certificate([-7, 0, 0, 0, 1], prime)
    assert irreducible and common == [1] and fourth == [0, 1]
    report = {
        "scope": "exact primality and extension irreducibility certificate; not protocol security",
        "prime": str(prime),
        "prime_minus_one_factors": factors,
        "lucas_generator": generator,
        "lucas_order_residues": residues,
        "extension_polynomial": "X^4 - 7",
        "gcd_with_x_to_p_squared_minus_x": common,
        "x_to_p_fourth_mod_polynomial": fourth,
        "source_sha256": {
            str(path.relative_to(root)): sha256(path.read_bytes()).hexdigest()
            for path in [field_path, params_path]
        },
    }
    output = root / "target/fastpq-production-validation/compact-field-certificate.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print("PASS: Goldilocks prime and X^4-7 irreducibility certified by exact arithmetic")
    print(output)


if __name__ == "__main__":
    if not __debug__:
        raise SystemExit("Certificate checks require Python assertions; remove -O/PYTHONOPTIMIZE")
    main()
