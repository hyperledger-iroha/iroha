#!/usr/bin/env python3
"""Reproduce native40 nonzero arithmetic vectors with independent integer math.

This is a test oracle, not a prover, transcript, or security qualification.
No Rust code is executed. Only the canonical prime inputs are read from source;
field operations, extension roots, polynomials, quotients, batching and folds
are evaluated below with Python arbitrary-precision integers. Inversion uses
the conjugate/norm formula rather than the implementation's extension power.
"""

import argparse
from pathlib import Path
import re


def vectors(root):
    """Return forty exact 37-word cases in canonical limb order."""
    base = root / "crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe"
    manifest = (base / "manifest.rs").read_text()
    source = manifest.split("const RELEASE_MODULI_V1: [u64; 38] = [", 1)[1].split("]", 1)[0]
    moduli = [int(word.replace("_", "")) for word in re.findall(r"\b[0-9][0-9_]*\b", source)]
    assert len(moduli) == 38
    profile = (base / "rns_native_profile.rs").read_text()
    for suffix in ("", " + 1"):
        match = re.search(r"values\[RELEASE_MODULI_V1\.len\(\)" + re.escape(suffix) + r"\] = ([0-9_]+);", profile)
        assert match
        moduli.append(int(match.group(1).replace("_", "")))
    result = []
    for limb, q in enumerate(moduli):
        nr = next(n for n in range(2, 65) if pow(n, (q - 1) // 2, q) == q - 1)
        def add(a, b):
            return ((a[0] + b[0]) % q, (a[1] + b[1]) % q)
        def sub(a, b):
            return ((a[0] - b[0]) % q, (a[1] - b[1]) % q)
        def mul(a, b):
            return ((a[0] * b[0] + nr * a[1] * b[1]) % q,
                    (a[0] * b[1] + a[1] * b[0]) % q)
        def power(a, exponent):
            value = (1, 0)
            for bit in bin(exponent)[2:]:
                value = mul(value, value)
                if bit == "1":
                    value = mul(value, a)
            return value
        def inverse(a):
            norm = (a[0] * a[0] - nr * a[1] * a[1]) % q
            assert norm
            reciprocal = pow(norm, -1, q)
            return (a[0] * reciprocal % q, -a[1] * reciprocal % q)
        order = 1 << 19
        candidates = (power((a, b), (q*q - 1) // order)
                      for a in range(1, 33) for b in range(1, 33))
        domain_root = next(r for r in candidates
                           if power(r, order) == (1, 0) and power(r, order // 2) != (1, 0))
        x = power(domain_root, 17 + 2 * limb)
        a = 17 + limb
        coefficients = [5 + limb, 7 + 2*limb, 11 + 3*limb, 13 + 5*limb]
        def g(t):
            return tuple(sum(coefficients[j] * power(t, j)[axis] for j in range(4)) % q
                         for axis in range(2))
        def f(t):
            return mul(add(power(t, 1 << 17), (1, 0)), g(t))
        eval_f, eval_g = f((a, 0)), g((a, 0))
        assert eval_f[1] == eval_g[1] == 0
        points = [x, sub((0, 0), x)]
        values = [f(t) for t in points] + [g(t) for t in points]
        quotients = [mul(sub(value, evaluation), inverse(sub(t, (a, 0))))
                     for row, evaluation in enumerate((eval_f, eval_g))
                     for t, value in zip(points, values[2*row:2*row+2])]
        batch_a, batch_b, alpha = (q-17, 23), (29, q-31), (37, q-41)
        batches = []
        for row in range(2):
            for side, t in enumerate(points):
                index = row*2 + side
                degree_shift = row * (1 << 17)
                batches.append(add(mul(batch_a, mul(power(t, degree_shift), values[index])),
                                   mul(batch_b, mul(power(t, degree_shift + 1), quotients[index]))))
        # Interpolate the even/odd coefficients of the unique line through +/-x.
        inverse_two = (pow(2, -1, q), 0)
        folds = []
        for row in range(2):
            positive, negative = batches[2*row:2*row+2]
            even = mul(add(positive, negative), inverse_two)
            odd = mul(sub(positive, negative), inverse(mul((2, 0), x)))
            folds.append(add(even, mul(alpha, odd)))
        assert all(value != (0, 0) for value in values + quotients + batches + folds)
        words = [q, nr, *domain_root, *x, a, eval_f[0], eval_g[0]]
        words += [word for pair in values + quotients + batches + folds for word in pair]
        assert len(words) == 37 and all(0 <= word < q for word in words[1:])
        result.append(words)
    assert len(result) == 40
    return result


def main():
    """Print the Rust constant, or compare an existing generated constant."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", type=Path)
    args = parser.parse_args()
    cases = vectors(args.root)
    if args.check:
        text = args.check.read_text().split("// BEGIN GENERATED VECTORS\n", 1)[1]
        body = text.split("// END GENERATED VECTORS", 1)[0].split("= [", 1)[1]
        observed = [int(word.replace("_", "")) for word in re.findall(r"\b[0-9][0-9_]*\b", body)]
        assert observed == [word for case in cases for word in case], "nonzero vector drift"
        print("40 fields, 80 nonzero polynomial rows: independent arithmetic vectors match")
        return
    print("// BEGIN GENERATED VECTORS\nconst VECTORS: [[u64; 37]; 40] = [")
    for case in cases:
        print("    [")
        for offset in range(0, len(case), 4):
            print("        " + ", ".join(f"{word:_}" for word in case[offset:offset+4]) + ",")
        print("    ],")
    print("];\n// END GENERATED VECTORS")


if __name__ == "__main__":
    main()
