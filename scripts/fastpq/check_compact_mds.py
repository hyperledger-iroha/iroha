#!/usr/bin/env python3
"""Exact width-three/one-S-box linear-layer screens, not a digest security proof."""
from pathlib import Path
import argparse
import hashlib
import itertools
import json
import math
import struct

ROOT = Path(__file__).resolve().parents[2]
P = 2**64 - 2**32 + 1
FROZEN = 'd97552e693a324b96cc4149945aca538656dd14a6ba8500481ee3efff5fc6899'
FROZEN_ASSET = '7b30fe50faeddb74a7433bc65a43b7c6b46484b36eab5a65869e08894166dfde'
FROZEN_POSEIDON = '1cf89ff798f7b029462b278429aa44e36eb4cf61ef7032e8fb2813bed4dabb84'
E0 = [1, 0, 0]


def identity(n):
    return [[int(i == j) for j in range(n)] for i in range(n)]


def mul(a, b, p):
    return [[sum(x * y for x, y in zip(row, col)) % p
             for col in zip(*b)] for row in a]


def apply(a, v, p):
    return [sum(x * y for x, y in zip(row, v)) % p for row in a]


def power(a, n, p):
    out = identity(len(a))
    while n:
        if n & 1:
            out = mul(out, a, p)
        a = mul(a, a, p)
        n >>= 1
    return out


def det(a, p):
    if not a:
        return 1
    return sum((-1)**j * a[0][j] * det([r[:j] + r[j+1:] for r in a[1:]], p)
               for j in range(len(a))) % p


def basis(rows, p):
    a = [[v % p for v in row] for row in rows]
    if not a:
        return []
    rank = 0
    for col in range(len(a[0])):
        pivot = next((i for i in range(rank, len(a)) if a[i][col]), None)
        if pivot is None:
            continue
        a[rank], a[pivot] = a[pivot], a[rank]
        inv = pow(a[rank][col], -1, p)
        a[rank] = [v * inv % p for v in a[rank]]
        for i in range(len(a)):
            if i != rank:
                c = a[i][col]
                a[i] = [(x - c*y) % p for x, y in zip(a[i], a[rank])]
        rank += 1
        if rank == len(a):
            break
    return a[:rank]


def kernel(rows, n, p):
    r = basis(rows, p)
    pivots = [next(j for j, x in enumerate(row) if x) for row in r]
    out = []
    for free in range(n):
        if free not in pivots:
            v = [0] * n
            v[free] = 1
            for row, pivot in zip(r, pivots):
                v[pivot] = -row[free] % p
            out.append(v)
    return basis(out, p)


def inactive_space(a, rounds, p):
    return kernel([power(a, j, p)[0] for j in range(rounds)], 3, p)


def invariant_core(a, subspace, p):
    # Descending S <- S intersect A^-1 S. At most three strict dimension drops.
    current = basis(subspace, p)
    while current:
        orthogonal = kernel(current, 3, p)
        nxt = kernel(orthogonal + mul(orthogonal, a, p), 3, p)
        if nxt == current:
            return current
        assert len(nxt) < len(current)
        current = nxt
    return []


def eigenvector_in_space(a, subspace, p):
    # Every eigenvector in S belongs to its largest A-invariant subspace.
    core = invariant_core(a, subspace, p)
    if not core:
        return False
    if len(core) == 1:
        return True
    assert len(core) == 2 and p > 2
    pivots = [next(j for j, x in enumerate(row) if x) for row in core]
    # RREF basis coordinates of any member w are exactly w[pivots].
    images = [apply(a, v, p) for v in core]
    restricted = [[images[j][pivots[i]] for j in range(2)] for i in range(2)]
    trace = (restricted[0][0] + restricted[1][1]) % p
    discriminant = (trace*trace - 4*det(restricted, p)) % p
    return discriminant == 0 or pow(discriminant, (p-1)//2, p) == 1


def reference_algorithm_1(a, p):
    checks = []
    for i in [1, 2]:  # floor((t-s)/s), t=3, s=1.
        ai = power(a, i, p)
        s = inactive_space(a, i, p)
        scalar = ai == [[ai[0][0] * int(j == k) for k in range(3)] for j in range(3)]
        eigen = eigenvector_in_space(ai, s, p)
        invariant_at = [j for j in range(1, i+1)
                        if basis([apply(power(a, j, p), v, p) for v in s], p) == s]
        checks.append({'i': i, 'inactive_space_basis': s, 'scalar_power': scalar,
                       'base_field_eigenvector_in_inactive_space': eigen,
                       'invariant_space_exponents': invariant_at})
    return {'pass': all(not c['scalar_power'] and
                        not c['base_field_eigenvector_in_inactive_space'] and
                        not c['invariant_space_exponents'] for c in checks), 'checks': checks}


def algorithm_2(a, p):
    # With one active coordinate, I_s={0} and full_iota_space=Fp^3.
    # Minimal A-invariant space containing e0 is its degree-three Krylov span.
    v1 = apply(a, E0, p)
    v2 = apply(a, v1, p)
    vectors = [E0, v1, v2]
    rank = len(basis(vectors, p))
    return {'pass': rank == 3, 'rank': rank, 'krylov_basis': basis(vectors, p),
            'krylov_determinant_mod_p': det(vectors, p)}


def published_algorithm_1(a, p):
    # Computing the largest M-invariant subspace inside the inactive plane
    # directly yields the same zero/nonzero decision as primary decomposition.
    # Every invariant subspace splits into primary components by polynomial
    # projectors in M, and every component stays inside that subspace.
    core = invariant_core(a, [[0, 1, 0], [0, 0, 1]], p)
    return {'pass': not core, 'largest_inactive_invariant_subspace_basis': core}


def published_algorithm_3(a, p):
    candidates = []
    for r in range(2, 13):
        candidate = algorithm_2(power(a, r, p), p)
        if candidate['pass']:
            continue
        subspace = candidate['krylov_basis']
        mapped = basis([apply(a, v, p) for v in subspace], p)
        if mapped == subspace:
            candidates.append({'period': r, 'reason': 'already invariant'})
            continue
        active_rounds = [0]
        incompatible = None
        for j in range(1, r):
            subspace = basis([apply(a, v, p) for v in subspace], p)
            if any(v[0] for v in subspace):
                if len(basis(subspace + [E0], p)) == len(subspace):
                    active_rounds.append(j)
                else:
                    incompatible = j
                    break
        candidates.append({'period': r, 'incompatible_round': incompatible,
                           'active_rounds': active_rounds})
    found = [c for c in candidates if c.get('reason') != 'already invariant'
             and c['incompatible_round'] is None]
    return {'pass': not found, 'candidates': candidates, 'iterative_trails': found}


def check_matrix(a, p):
    assert p > 2 and len(a) == 3 and all(len(row) == 3 for row in a)
    if det(a, p) == 0:
        raise ValueError('GRS screens require an invertible matrix')
    minors = []
    for size in range(1, 4):
        for rows in itertools.combinations(range(3), size):
            for cols in itertools.combinations(range(3), size):
                value = det([[a[i][j] for j in cols] for i in rows], p)
                minors.append({'rows': rows, 'columns': cols, 'determinant_mod_p': value})
    powers = [{'exponent': i, **algorithm_2(power(a, i, p), p)} for i in range(1, 13)]
    return {'matrix': a, 'mds': all(c['determinant_mod_p'] for c in minors),
            'square_minors': minors, 'reference_algorithm_1': reference_algorithm_1(a, p),
            'published_algorithm_1': published_algorithm_1(a, p),
            'published_algorithm_3': published_algorithm_3(a, p),
            'algorithm_2': powers[0], 'active_invariant_screens_powers_1_to_12': powers,
            'algorithm_3_conservative_screen_pass': all(c['pass'] for c in powers[1:])}


def projective_points(p):
    # Independent exhaustive normalized representatives, no elimination helper.
    return [list(v) for v in itertools.product(range(p), repeat=3)
            if any(v) and next(x for x in v if x) == 1]


def brute_algorithm_1(a, p):
    points = projective_points(p)
    for i in [1, 2]:
        ai = power(a, i, p)
        if ai == [[ai[0][0] * int(j == k) for k in range(3)] for j in range(3)]:
            return False
        members = [v for v in points if all(apply(power(a, j, p), v, p)[0] == 0
                                           for j in range(i))]
        for v in members:
            if any(apply(ai, v, p) == [e*x % p for x in v] for e in range(p)):
                return False
        # Invertibility turns containment into equality for each tested space.
        for j in range(1, i+1):
            if all(all(apply(power(a, k+j, p), v, p)[0] == 0 for k in range(i))
                   for v in members):
                return False
    return True


def brute_active_invariant(a, p):
    # The proper spaces containing e0 are that line and all planes with normal
    # (0,b,c). Enumerate all their vectors and test closure, independently of rank.
    points = projective_points(p)
    candidates = [[v for v in itertools.product(range(p), repeat=3) if v[1:] == (0, 0)]]
    for normal in points:
        if normal[0] == 0:
            candidates.append([v for v in itertools.product(range(p), repeat=3)
                               if sum(x*y for x, y in zip(v, normal)) % p == 0])
    for members in candidates:
        values = set(members)
        if all(tuple(apply(a, v, p)) in values for v in members):
            return True
    return False


def brute_algorithm_3(a, p):
    for r in range(2, 13):
        ar = power(a, r, p)
        vectors = [E0, apply(ar, E0, p), apply(power(ar, 2, p), E0, p)]
        members = {tuple(sum(c*v[j] for c, v in zip(coefficients, vectors)) % p
                         for j in range(3))
                   for coefficients in itertools.product(range(p), repeat=3)}
        if len(members) == p**3:
            continue
        if {tuple(apply(a, v, p)) for v in members} == members:
            continue
        for _ in range(1, r):
            members = {tuple(apply(a, v, p)) for v in members}
            if any(v[0] for v in members) and tuple(E0) not in members:
                break
        else:
            return False
    return True


def controls():
    counts = {'matrices': 0, 'reference_algorithm_1_matches': 0, 'published_algorithm_1_matches': 0, 'algorithm_2_power_matches': 0, 'published_algorithm_3_matches': 0}
    # Exhaustive invertible binary-entry 3x3 matrices over F3, plus a
    # deterministic full-alphabet F5 sample; no external algebra dependency.
    cases = [(3, [list(v[i:i+3]) for i in [0, 3, 6]])
             for v in itertools.product(range(2), repeat=9)]
    seed = 0x5A17
    for _ in range(64):
        values = []
        for _ in range(9):
            seed = (1664525*seed + 1013904223) % 2**32
            values.append(seed % 5)
        cases.append((5, [values[i:i+3] for i in [0, 3, 6]]))
    for p, a in cases:
        if det(a, p) == 0:
            continue
        assert reference_algorithm_1(a, p)['pass'] == brute_algorithm_1(a, p)
        counts['reference_algorithm_1_matches'] += 1
        inactive_survivors = [v for v in projective_points(p)
                              if all(apply(power(a, j, p), v, p)[0] == 0 for j in range(3))]
        assert published_algorithm_1(a, p)['pass'] == (not inactive_survivors)
        counts['published_algorithm_1_matches'] += 1
        for i in range(1, 13):
            ai = power(a, i, p)
            assert algorithm_2(ai, p)['pass'] == (not brute_active_invariant(ai, p))
            counts['algorithm_2_power_matches'] += 1
        assert published_algorithm_3(a, p)['pass'] == brute_algorithm_3(a, p)
        counts['published_algorithm_3_matches'] += 1
        counts['matrices'] += 1
    # A scalar matrix fails every screen; a 3-cycle passes one-step active
    # cyclicity but fails period three. The symmetric matrix is MDS yet insecure:
    # symmetry gives the inactive eigenvector (0,1,-1).
    scalar = identity(3)
    cycle = [[0, 1, 0], [0, 0, 1], [1, 0, 0]]
    symmetric = [[2, 1, 1], [1, 2, 1], [1, 1, 2]]
    irreducible_inactive = [[1, 0, 0], [0, 0, 7], [0, 1, 0]]
    assert pow(7, (P-1)//2, P) == P-1
    assert not eigenvector_in_space(irreducible_inactive, [[0, 1, 0], [0, 0, 1]], P)
    assert not published_algorithm_1(irreducible_inactive, P)['pass']
    assert not reference_algorithm_1(irreducible_inactive, P)['pass']
    sc = check_matrix(scalar, P)
    cy = check_matrix(cycle, P)
    sy = check_matrix(symmetric, P)
    assert not sc['published_algorithm_1']['pass'] and not sc['algorithm_2']['pass']
    assert cy['published_algorithm_1']['pass'] and cy['algorithm_2']['pass']
    assert not cy['active_invariant_screens_powers_1_to_12'][2]['pass']
    assert not cy['published_algorithm_3']['pass']
    assert sc['published_algorithm_3']['pass']  # Invariant trails are Algorithm 2's responsibility.
    assert sy['mds'] and not sy['published_algorithm_1']['pass'] and not sy['algorithm_2']['pass']
    # x^3+x+1 has no root in F5, hence is irreducible of degree three.
    assert all((x**3+x+1) % 5 for x in range(5))
    companion = check_matrix([[0, 0, 4], [1, 0, 4], [0, 1, 0]], 5)
    assert companion['published_algorithm_1']['pass']
    assert companion['reference_algorithm_1']['pass']
    assert companion['published_algorithm_3']['pass']
    assert all(c['pass'] for c in companion['active_invariant_screens_powers_1_to_12'])
    try:
        check_matrix([[0]*3 for _ in range(3)], P)
    except ValueError:
        pass
    else:
        raise AssertionError('singular matrix accepted')
    return {'enumeration': counts, 'scalar': sc, 'three_cycle': cy, 'mds_but_inactive_eigenvector': sy,
            'irreducible_inactive_plane': check_matrix(irreducible_inactive, P),
            'positive_irreducible_companion_over_F5': companion}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path,
        default=ROOT / "target/fastpq-production-validation/compact-mds-linear-checks-certificate.json",
        help="generated JSON evidence path (not a security qualification)",
    )
    args = parser.parse_args()
    digest = ROOT / 'crates/fastpq_isi/src/poseidon_digest384.rs'
    assert hashlib.sha256(digest.read_bytes()).hexdigest() == FROZEN
    asset = ROOT / 'crates/fastpq_isi/src/assets/poseidon_goldilocks_width3_v1.bin'
    assert hashlib.sha256(asset.read_bytes()).hexdigest() == FROZEN_ASSET
    poseidon = ROOT / 'crates/fastpq_isi/src/poseidon.rs'
    assert hashlib.sha256(poseidon.read_bytes()).hexdigest() == FROZEN_POSEIDON
    words = struct.unpack('<204Q', asset.read_bytes())
    a = [list(words[195 + 3*i:198 + 3*i]) for i in range(3)]
    assert all(0 <= x < P for row in a for x in row)
    result = check_matrix(a, P)
    assert result['mds'] and result['reference_algorithm_1']['pass']
    assert result['published_algorithm_1']['pass'] and result['published_algorithm_3']['pass']
    assert not result['published_algorithm_3']['candidates']
    assert all(c['pass'] for c in result['active_invariant_screens_powers_1_to_12'])
    assert math.gcd(7, P-1) == 1 and P > 7
    tests = controls()
    sources = [digest, asset, poseidon, Path(__file__)]
    # Optional retained primary-source copies enrich provenance; a repository
    # checkout does not depend on the local research downloads.
    for name in ['grs2021-linear.pdf', 'poseidon2021.pdf', 'poseidon2_rust_params.sage']:
        source = ROOT / 'target/fastpq-production-validation/soundness-paper-text' / name
        if source.exists():
            sources.append(source)
    record = {'status': 'PASS: width-three linear-layer screens only; no digest security qualification',
              'prime': P, 'width': 3, 'active_coordinates': [0], 'max_period': 12,
              'source_sha256': {str(f.relative_to(ROOT)): hashlib.sha256(f.read_bytes()).hexdigest() for f in sources},
              'frozen_matrix': result, 'controls': tests,
              'scope': ['Published GRS21 Algorithm 1 decision via exact largest inactive invariant subspace; older reference Algorithm 1 also checked',
                        'GRS Algorithm 2 on M^1 through M^12',
                        'Published GRS21 Algorithm 3, period 12, and stronger conservative primary reference screen; no actual-matrix candidate survives to later trail checks',
                        'GRS word-level no-nontrivial-linear-structure premise: a prime field has no proper nonzero additive subspace; additionally x^7 has no nonzero constant derivative since coefficient 7a != 0'],
              'not_established': ['full 65-round permutation security', 'six-lane combiner binding/collision/quantum security',
                                  'security against every subspace or algebraic attack', 'production qualification']}
    out = args.output
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(record, indent=2) + '\n')
    print(record['status'])
    print(json.dumps(tests['enumeration']))
    print(out)


if __name__ == '__main__':
    if not __debug__:
        raise SystemExit('Refusing optimized Python: validation assertions must run')
    main()
