# Conditional FRI-only bound for the current compact geometry

Reviewed conditional component, 2026-09-06. This derives a bound for
the interactive FRI component under the explicit premises below. It does not
qualify the FASTPQ profile, prove a false AIR statement produces the required
distance, instantiate a hash assumption, or prove Fiat–Shamir/qROM security.
No Rust, profile or admission change is proposed.

**Result.** Suppose the committed initial FRI oracle is at relative Hamming
distance **strictly greater than 49/100** from the degree-below-131,072
Reed–Solomon code on the current 524,288-point coset. With perfectly bound
oracles, independent uniform challenges in `K=F_p^4`, and a fresh uniform
136-element subset of initial query positions, the current 17-fold FRI check
has the following conditional interactive acceptance bound:

```
err_commit <= (13775589115872148 / 5) / p^4 < 2^-204
err_query  <= binom(267386,136) / binom(524288,136)
err_total  <= err_commit + err_query < 2^-132,
p = 2^64 - 2^32 + 1.
```

The two errors are added. Commit error is not exponentiated by the number of
query positions. This numerical example depends critically on its distance
premise; it says nothing equivalent for a word merely known not to be a
codeword.

## Primary theorem and exact protocol correspondence

The substantive imported result is Ben-Sasson, Carmon, Haböck, Kopparty and
Saraf, [*On Proximity Gaps for Reed–Solomon Codes*](https://www.math.toronto.edu/swastik/rs-proximity-gaps-2025.pdf),
November 11, 2025 manuscript, **Theorem 4.2 and Corollary 4.4, pp. 27–28**.
The latter is a weighted correlated-agreement theorem for a sub-probability
measure `mu({y})=w(y)/n`, `0<=w(y)<=1`. It applies to an arbitrary evaluation
set in a finite field and to the affine line `u0+beta*u1` (`M=1` here).
No random-evaluation-set assumption or near-capacity conjecture is used.

In that paper the degree bound `k` is inclusive and its slightly reduced rate
is `rho=k/n`. For `0<gamma<1-sqrt(rho)`, put

```
m = max(ceil(sqrt(rho)/(1-sqrt(rho)-gamma)), 3)
h = m + 1/2
B(n,rho,gamma) = [2*h^5 + 3*h*gamma*rho] * n/(3*rho^(3/2))
                 + h/sqrt(rho).
```

Theorem 4.2's displayed, conservative choice of `m` is used here. Theorem 1.5
elsewhere in the manuscript has a smaller denominator-dependent choice; the
calculation does not rely on that improvement. Corollary 4.4 says that if more
than `B` scalars have a codeword agreeing with `u0+beta*u1` on weight at least
`1-gamma`, then one pair of codewords agrees simultaneously with `u0,u1` on
weight at least `1-gamma`. Its contrapositive bounds the number of exceptional
scalars by `B`, not by a heuristic probability.

The original FRI paper, Ben-Sasson, Bentov, Horesh and Riabzev,
[*Fast Reed-Solomon Interactive Oracle Proofs of Proximity*](https://drops.dagstuhl.de/storage/00lipics/lipics-vol107-icalp2018/LIPIcs.ICALP.2018.14/LIPIcs.ICALP.2018.14.pdf),
§2.1, describes the same smooth multiplicative-domain even/odd fold. Its
single-query Theorem 2 is not being raised to the 136th power here. Likewise,
the informal constants in §4.1/Theorem 4 of
[*DEEP-FRI*](https://drops.dagstuhl.de/storage/00lipics/lipics-vol151-itcs2020/LIPIcs.ITCS.2020.5/LIPIcs.ITCS.2020.5.pdf)
are not needed, and no DEEP-FRI soundness theorem is imported.

The source contract and shared verifier have domains and exclusive bounds

```
D_i = {x^(2^i) : x in D_0},
n_i = |D_i| = 524288 / 2^i,
d_i = 131072 / 2^i,                  0 <= i <= 17.
```

Every `D_i` is a nonzero coset and `x -> x^2` is two-to-one before the
terminal. For a committed word `f_i:D_i->K`, define words on `D_(i+1)` by

```
e_i(x^2) = (f_i(x)+f_i(-x))/2
o_i(x^2) = (f_i(x)-f_i(-x))/(2*x).
```

Both are independent of which square root is named `x`. The checked fold is
exactly `e_i+beta_i*o_i`. A pair of degree-`<d_(i+1)` polynomials lifts to
`P(X)=P0(X^2)+X*P1(X^2)` of degree `<d_i`. Squaring the actual coset offset
at each round makes this a coordinate identity, without an additional domain
assumption. At `i=17`, the code is the constant code on all four terminal
points, and the verifier checks the whole terminal vector is constant.

## Assumptions and adaptive chronology

1. `f_0` is fixed before `beta_0`, with the stated distance. It is the oracle
   represented by the initial FRI root. A distance statement about a separately
   computed joint expression `Q+(rho+sigma*X^N)*T` is insufficient unless the
   missing linkage argument transfers that statement to this committed oracle.
2. All roots bind complete single-valued words; openings cannot equivocate.
   This is the ideal interactive-oracle premise. Concrete Merkle/hash failure
   probabilities have not been included in the numerical bound.
3. At each round `f_i` and all earlier oracles are fixed before an independent
   uniform `beta_i` in `K`. After seeing it the prover may choose arbitrary
   `f_(i+1)`, adaptively. The proof below permits this adaptivity.
4. After all oracles are fixed, query positions form a uniform subset of 136
   distinct positions, using randomness fresh relative to these commitments.
   For the actual bounded sampler this follows only under the separate iid
   ideal-digest premises in [the sampler lemma](fastpq_compact_query_sampler.md). Abort
   rejects, so it adds no false-acceptance term for one attempt. Selecting an
   attempt after observing its query positions is outside these assumptions.

Canonical preflight, additional AIR checks and initial joint-value checks can
only reject further when the premises hold. This does not prove their own
security reduction or the premises themselves.

## Prefix-survival invariant

This section supplies the composition argument rather than assuming that
proximity is preserved under every maliciously chosen next word.

Initially give every initial position mass `1/n_0`. At round `i`, let `mu_i(x)`
be the total initial mass that maps to `x` and has passed every earlier fold
edge. Thus `mu_0(x)=1/n_0`; define the next measure by

```
nu_i(y) = mu_i(x)+mu_i(-x),             y=x^2,
mu_(i+1)(y) = nu_i(y) * 1[f_(i+1)(y)=e_i(y)+beta_i*o_i(y)].
```

Inductively `0<=mu_i(x)<=1/n_i` and `0<=nu_i(y)<=1/n_(i+1)`. Both `mu_i`
and `nu_i` are fixed before `beta_i`. They are exactly measures allowed by
Corollary 4.4; their mass may decrease, and their weights need not be uniform.

Write `C_i` for the degree-`<d_i` code and set

```
P_i = max_{g in C_i} sum_x mu_i(x)*1[f_i(x)=g(x)],
a = 1-gamma.
```

Suppose `P_i<a`. If polynomials `P0,P1` agreed simultaneously with `e_i,o_i`
on a set of `nu_i`-weight at least `a`, their lifted polynomial would agree
with `f_i` at both square roots on `mu_i`-weight at least `a`, contradicting
`P_i<a`.

For any chosen next oracle, `P_(i+1)>=a` implies that some `g in C_(i+1)`
agrees with the genuine folded word `e_i+beta_i*o_i` on `nu_i`-weight at least
`a`: take the agreement set realizing `P_(i+1)` and retain its positive-weight
points. At every such point the fold edge passed. Therefore Corollary 4.4's
contrapositive bounds the number of challenges permitting `P_(i+1)>=a` by
`B(n_(i+1), (d_(i+1)-1)/n_(i+1), gamma)`, independently of how the prover
selects its next oracle after the challenge.

Initially `P_0=1-distance(f_0,C_0)<a`. Applying the preceding conditional
bound at the first crossing of the threshold and taking a union bound over
rounds separates the exceptional commit event from the later query event.
There is no accumulated distance-loss term in this invariant.

## The constant-code terminal fold

The displayed paper formula divides by `rho`; it must not be substituted at
the last fold, where `n_17=4`, `d_17=1`, and `rho=0`.

An elementary exception count replaces that step. Fix `e,o` on the four
points and a measure `nu` with atoms at most `1/4`. Suppose no constant pair
`(c0,c1)` agrees with `(e,o)` on weight at least `a`. If some scalar `beta`
makes `e+beta*o` agree with a constant on a set `A` of weight at least `a`,
then `A` contains two points `y,z` with `(e(y),o(y)) != (e(z),o(z))`.
Otherwise the forbidden constant pair would agree on all of `A`. Those two
points obey

```
e(y)-e(z) + beta*(o(y)-o(z)) = 0.
```

For a distinct pair of vectors this equation has at most one solution for
`beta` (and none if its linear coefficient vanishes). There are only
`binom(4,2)=6` pairs. Hence at most six challenges allow the crossing in the
last fold. No interpolation or `rho=0` limiting argument is assumed.

If the complete terminal vector is nonconstant, verification rejects. If it
is constant, `P_17` equals the entire surviving mass. Outside the exceptional
commit event this mass is strictly less than `a`. Consequently fewer than
`a*n_0` original positions pass all fold checks. Merged descendants do not
create independent trials and do not invalidate this fixed-set statement.

## Explicit conservative constants at gamma=49/100

For the first 16 folds the next lengths range from 262,144 down to 8 and
the next inclusive code degree is `k=n/4-1`. Thus

```
1/8 <= rho < 1/4,
1/3 < sqrt(rho) < 1/2,
1-sqrt(rho)-gamma > 1/100,
m <= 50,       h <= 101/2,
3*rho^(3/2) > 1/8.
```

In particular every invocation lies strictly within the theorem's proven
Johnson-radius range. These inequalities give the uniform rational ceiling

```
B(n,rho,49/100) < C*n + 152,
C = 8 * [2*(101/2)^5 + 3*(101/2)*(49/100)*(1/4)]
  = 525505039897/100.
```

The first 16 next-domain lengths sum to 524,280. Including the elementary
terminal contribution therefore gives

```
sum(B_i) < C*524280 + 16*152 + 6
         = 13775589115872148/5.
```

Every beta has `|K|=p^4` equally likely possibilities by assumption. Union
bounding first-threshold-crossing events gives the stated commit bound.

On a good commit transcript, the integer size of the passing set is at most
`ceil((51/100)*524288)-1=267386`. Sampling without replacement gives exactly
`binom(|A|,136)/binom(524288,136)` for all 136 queries to land in that set.
Monotonicity in `|A|` gives the stated hypergeometric bound. The looser bound
`(51/100)^136` would also work here; it is applied only to the fixed passing
set after excluding commit failures, not to an unconditional single-query
FRI bound. The integer/rational certificate verifies the displayed total is
strictly below `2^-132`.

## Reproduction and remaining boundary

Run `python3 scripts/fastpq/check_compact_fri_bound.py` from the repository.
It verifies geometry, the rational majorants, the hypergeometric fraction,
and strict dyadic bounds without floating-point arithmetic, and emits a local JSON record under `target/fastpq-production-validation/`
including the reviewed primary PDF identity. The certificate checks
the arithmetic; it does not machine-prove Corollary 4.4 or this composition
argument. Independent source and mathematical review checked the imported
theorem hypotheses, adaptive weighted invariant, terminal exceptions and every
reported fraction without finding a concrete defect. This is internal review
of the conditional lemma, not independent release qualification.

No protocol change is needed for this narrow ideal-oracle FRI lemma. Its
missing premises are substantial: proximity of the committed joint oracle
from a false AIR instance; correlated row/constraint/joint linkage; the exact
interactive-to-Fiat–Shamir transformation; quantum/random-oracle work and
multiple-attempt accounting; and concrete commitment/transcript security.
The 132-bit number is an illustrative conditional interactive FRI error,
not FASTPQ security bits or evidence that 136 queries suffice for production.
