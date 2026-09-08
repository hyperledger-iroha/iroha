# Conditional interactive AIR reduction for the current compact openings

Reviewed conditional component, 2026-09-06. The derivation and exact arithmetic
received separate internal reviews. This is not independent external security
qualification. No Rust, profile or admission changes accompany this document.

**Result.** A conservative reduction is possible with the existing current-row
mixed opening, next-row opening, joint `Q,T,X^N*T` batching and binary FRI.
Its useful agreement threshold is **11/16**, rather than the **0.51** used in
the separate FRI-only illustration. For an unsatisfiable formal AIR instance,
342 columns, `N=65,536`, `L=8N=524,288`, degree-`<N` trace columns,
degree-`<2N` quotient/joint code, and numerator degree `<3N`, the conditional
interactive acceptance bound derived below is

```
err(q) <= (8065872632161 / 2) / p^4
          + binom(360447,q) / binom(524288,q),
p = 2^64 - 2^32 + 1.
```

The commit-error majorant is `<2^-214`. At **237 distinct initial queries**,
the sum is `<2^-128`, verified with exact rational arithmetic. At the current
136 queries this proof supplies only approximately `2^-73.53` for its query
term. That weaker upper bound is not an attack or a claim of tight security.
The number 237 is an illustrative **interactive** parameter, not a proposed
production profile: Fiat–Shamir/qROM, concrete hashes, repeated attempts,
semantic AIR review and resource qualification remain unaccounted for.

## Exact assumptions and scope of “false”

The relation in this theorem consists of the source's fixed, public-statement
dependent numerators `C_1,...,C_m`. “False” means there is **no** vector `P` of
base-field polynomials, each degree `<N`, such that

```
C_k(h, P(h), P(omega*h)) = 0
for every h in H and every k,        omega = g^8.
```

The [semantic source audit](fastpq_compact_air_semantics.md) gives the
satisfaction-to-computation argument for the current transfer/SMT/AXT relation,
including padding and public checks. External review remains required.
This document does not assume that a merely incorrect public description is
automatically an unsatisfiable AIR. The AIR degree obligation is that each
substituted numerator has degree at most `3N-1`. The
[source degree ledger](fastpq_compact_air_degree_ledger.md) establishes this for
the current ordinary/AXT evaluator: hash slots have degree at most 196478 and
SMT slots at most 131070. The Rust trait alone does not prove it; evaluator or
fixed-polynomial changes require renewed review.

Further premises are: `K=F_(p^4)` is a field; all oracle commitments bind
complete single-valued words; row cells are canonical elements of `F_p`;
the source cosets and square maps are correct; all column, numerator, joint
and FRI challenges are independent uniform elements of `K` when sent; and
the final query positions are a fresh uniform subset of size `q` after the
commit phase. These are ideal interactive-oracle assumptions, not consequences
of concrete transcript framing. No proximity premise on a malicious oracle
is assumed. Sampler abort rejects and therefore adds no false-acceptance term
for one unselected attempt under the separate ideal-sampler premises.

The exact checker `scripts/fastpq/check_compact_field.py` verifies a Lucas
primality certificate for `p` and the Rabin irreducibility criterion for
`X^4-7`, using independent polynomial arithmetic. Thus the field premise has
an explicit arithmetic certificate. It supplies no commitment/hash assumption.

## Imported theorem and an independent-coefficient corollary

The sole proximity theorem imported here is the weighted correlated-agreement
result, **Theorem 4.2 and Corollary 4.4, pp. 27–28**, in Ben-Sasson, Carmon,
Haböck, Kopparty and Saraf,
[*On Proximity Gaps for Reed–Solomon Codes*](https://www.math.toronto.edu/swastik/rs-proximity-gaps-2025.pdf),
November 11, 2025 manuscript. This is the same reviewed PDF used by the adjacent
FRI-only draft. It applies to fixed arbitrary evaluation sets and the affine
line `u0+z*u1`; no DEEP step, random evaluation set or conjecture is used.

For `C=RS[K,D,k]`, the paper's degree `k` is inclusive and `rho=k/|D|`. Set
`a=11/16`, `gamma=1-a=5/16`, and

```
m = max(ceil(sqrt(rho)/(1-sqrt(rho)-gamma)), 3)
h = m+1/2
B(n,rho) = [2*h^5+3*h*gamma*rho]*n/(3*rho^(3/2)) + h/sqrt(rho).
```

For any fixed sub-probability measure `mu(x)<=1/n`, if no pair of codewords
agrees jointly with `u0,u1` on `mu`-weight at least `a`, at most `B(n,rho)`
scalars produce a combination agreeing with a codeword on weight at least
`a`. All applications below have `gamma<1-sqrt(rho)`.

**Uniqueness.** If `2a-1>k/n`, any word has at most one codeword agreeing on
`mu`-weight at least `a`. Two such agreement sets would intersect on weight
at least `2a-mu(D)>=2a-1`, hence on more than `k` points, forcing equality.
The condition holds for both codes used here (`k=N-1` and `k=2N-1`).

**Independent-coefficient lemma.** For fixed words `u0,...,us` and fixed
`mu`, assume no vector of codewords agrees jointly with all words on weight
at least `a`. For independent uniform `z_1,...,z_s`,

```
Pr[there exists c in C agreeing with u0+sum_j z_j*u_j
   on mu-weight >=a] <= s*B(n,rho)/|K|.                 (1)
```

Proof by induction on `s`: write `W=u0+sum_(j<s) z_j*u_j`. If `u_s` has
no codeword with agreement weight `a`, the affine-line theorem bounds the
last challenge by `B/|K|` for every fixed `W`. Otherwise its codeword `p_s`
is unique. Restrict the fixed measure to `nu=mu*1[u_s=p_s]`. There is no
joint agreement of the first `s` words under `nu`, since it would give joint
agreement of all words under `mu`. The inductive bound controls the event
that `W` has code agreement weight `a` under `nu`. Outside that event,
`W,u_s` cannot have joint agreement weight `a` under `mu`: any candidate
for `u_s` would be the unique `p_s`. Apply the affine-line theorem to the
last challenge and add the two error terms. The base case is Corollary 4.4.
If `mu(D)<a` the statement is immediate. This argument supplies the mapping
to independent coefficients; it does not cite a curve theorem as though the
source sampled powers of one scalar.

## Chronology and the two row events

The implementation commits the entire row oracle `A:D->F_p^342`, samples
`lambda`, commits `T`, samples `alpha`, commits `Q`, samples `rho,sigma`,
then commits the FRI layers with their intervening beta challenges. Define
the fixed-after-lambda row mixture

```
R_lambda(x) = sum_j lambda_j*A_j(x).
```

Call a polynomial row vector `P` a candidate if it agrees with `A` on a
**common** set of size at least `aL`. There is at most one such vector:
intersection of two agreement sets has at least `(2a-1)L=3N` points, which
exceeds every column degree. If the candidate exists, it is determined by
`A` alone, before lambda and alpha. Each column has at least `N` base-field
agreement points, so the coefficientwise Frobenius/root-count argument from
[the row-linkage lemma](fastpq_compact_row_linkage.md) makes `P` a base-field vector.

There are two different events to exclude:

1. **Missing common row despite a close mixture.** If `A` has no candidate,
   (1), with `u0=0`, `s=342`, the degree-`<N` code, and uniform measure,
   bounds the probability that `R_lambda` agrees with any such polynomial on
   `aL` points by `342*B_row/|K|`. A subsequently chosen `T` cannot change
   this assertion about the already fixed mixture.
2. **Cancellation of an incorrect whole row.** If the candidate `P` exists,
   it was fixed before lambda. For every `x` with `A(x)!=P(x)`, the nonzero
   vector difference has dot product zero with uniform lambda with probability
   exactly `1/|K|`. Union bounding all `L` positions costs at most `L/|K|`.
   Outside this event,

   ```
   R_lambda(x) = sum_j lambda_j*P_j(x)  implies  A(x)=P(x)
   at every x in D.                                    (2)
   ```

The second event is needed: common agreement somewhere does not by itself
turn a sampled equality of mixed values into equality of the whole row at
that position. Conditioning on an alpha-dependent recovered polynomial would
invalidate this probability bound; uniqueness from `A` avoids that problem.

## Numerator challenge event

For a false formal AIR, the candidate `P`, if present, violates at least one
numerator at some `h in H`. Choose a violating `h` deterministically from
`P` before alpha is sampled. The numerator vector at that `h` is nonzero,
so its dot product with independent uniform alpha vanishes with probability
exactly `1/|K|`. Thus, except with that probability, **every** degree-`<2N`
polynomial `q` has a nonzero residual

```
R_alpha(X) = sum_k alpha_k*C_k(X,P(X),P(omega*X))
             - (X^N-1)*q(X).                           (3)
```

The quotient polynomial may be selected after alpha. It does not affect the
argument at `h`, because `h^N-1=0`. There is no factor 923 from the number
of slots. The residual degree is at most `3N-1`.

## Joint batching on the actual pre-joint passing set

After `A,lambda,T,alpha,Q` are fixed and before `rho,sigma`, define

```
E_pre = {x in D:
  T(x)=R_lambda(x),
  Q(x)=sum_k alpha_k*C_k(x,A(x),A(omega*x))/(x^N-1)}
mu(x) = 1[x in E_pre]/L.
```

This is a legitimate fixed sub-probability measure for the next challenges.
It uses exactly the source's current-row mix and current/next quotient checks;
it does not insert a nonexistent `T(omega*x)` check. Source `D` is disjoint
from `H`, so every displayed denominator is nonzero.

The joint word is

```
J = Q + rho*T + sigma*U,         U(x)=x^N*T(x).
```

Apply (1) to `u0=Q,u1=T,u2=U`, degree-`<2N` code, and the fixed measure
`mu`. Except with probability at most `2*B_joint/|K|`, agreement of `J`
with a codeword on `mu`-weight at least `a` implies degree-`<2N` polynomials
`q,t,s` and a common set `S subset E_pre`, `|S|>=aL`, such that

```
Q=q,      T=t,      U=s          on S.                  (4)
```

Since `U=X^N*T` pointwise, `s-X^N*t` vanishes on `S`. It has degree at most
`3N-1`, while `|S|>=aL=11N/2>3N-1`. Therefore `s=X^N*t` as polynomials,
and `deg(s)<2N` forces `deg(t)<N`. No quotient-ring wraparound is used in
this step: recovered `t` has degree `<2N`, so `X^N*t` has degree `<3N<L`.

Because `S subset E_pre`, the mixture `R_lambda` equals this degree-`<N`
polynomial `t` on at least `aL` points. Outside row event 1, the unique
candidate `P` must therefore exist. Let `G` be its common agreement set.
On `S intersection G`, both `t` and `sum lambda_j*P_j` equal the mixture.
The intersection has at least `(2a-1)L=3N>=N` points, hence these two
degree-`<N` polynomials are identical. Outside row event 2, (2) now gives

```
A(x)=P(x) at every x in S.                             (5)
```

This is the place where the common set for the actual quotient test is
established. It is stronger than merely knowing unrelated global proximity
sets for each column and for Q.

## The next-row intersection and why 0.51 does not suffice

Although the verifier opens `A(omega*x)`, it does not mix-link that row at
the same query. Using only verified common rows, set

```
E = S intersection omega^(-1)*S.
```

The shift is a permutation of the same domain, so

```
|E| >= 2*|S|-L >= (2a-1)L = 3N.                        (6)
```

For every `x in E`, both current and next rows equal `P`, Q equals `q`, and
the actual quotient equality holds. Hence (3) vanishes on `E`. It has at
least `3N` roots and degree at most `3N-1`, contradicting the nonzero
residual outside the numerator event.

Consequently, except for the enumerated commit events, **no** degree-`<2N`
polynomial can agree with `J` on `mu`-weight at least `a=11/16`.

For an agreement threshold 0.51, this same intersection reasoning guarantees
only 0.02L points, far below the `<3N` degree budget. The high-distance premise
used in the earlier FRI-only result is therefore not supplied by this AIR
reduction. Even before the shift issue, 0.51 is below the simple uniqueness
thresholds used in the independent-coefficient lemma. Raising only that earlier
FRI query bound to an AIR claim would omit essential premises.

An extra authenticated `T(omega*x)` value and equality with the next row's
mixture would not alone prove it equals the recovered polynomial `t(omega*x)`:
an authenticated word may differ from its recovered polynomial on a bad set.
A protocol seeking a materially smaller agreement threshold needs a reviewed
low-degree evaluation/linkage mechanism for the required neighboring values,
or a stronger correlated analysis. This draft does not claim that one extra
Merkle opening removes the intersection loss. **No such change is needed for
the conservative high-threshold theorem just proved.**

## Weighted initial FRI state and query repetition

After the initial FRI oracle `f0` is committed, initialize the prefix-survival
measure from the adjacent FRI proof by

```
mu_0(x) = mu(x)*1[f0(x)=J(x)].
```

It is fixed before `beta_0`. A codeword agreeing with `f0` under `mu_0` on
weight at least `a` would agree with `J` under `mu` on at least that weight,
contradicting the preceding result. Thus the initial weighted potential is
strictly below `a`, without assuming that the unweighted `f0` itself is far.

The prefix-survival induction in [the conditional FRI lemma](fastpq_compact_fri_bound.md) applies
unchanged to this sub-probability initial measure: push surviving mass through
each two-to-one fold, remove failed edges, and bound any first threshold
crossing by the weighted theorem. The last constant-code fold has at most six
exceptional challenges. If the entire terminal vector is nonconstant the
verifier rejects; if it is constant, the terminal potential equals the mass
of initial positions passing **all** the included checks. Outside the FRI
commit event this mass is `<a`, so its integer cardinality is at most

```
ceil(aL)-1 = 360447.
```

The exact query acceptance bound is therefore hypergeometric. Shared descendant
groups are not independent trials; the proof counts the fixed passing set of
original positions. All commit failures are added once, outside that query
probability.

## Conservative explicit constants

The next-code reduced rates in the first 16 FRI folds lie in `[1/8,1/4)`;
the joint code also lies there. At `gamma=5/16`, `m<=3`, so `h<=7/2`.
Using `sqrt(rho)>1/3` gives

```
B(n,rho) < C_F*n+11,
C_F = 8*[2*(7/2)^5+3*(7/2)*(5/16)*(1/4)] = 134561/16.
```

The row code has `rho=(N-1)/L` in `[1/16,1/8)`. Using
`sqrt(rho)>1/4`, `sqrt(rho)<3/8`, and again `m<=3`, gives

```
B_row < C_R*L+14,
C_R = (64/3)*[2*(7/2)^5+3*(7/2)*(5/16)*(1/8)] = 269017/12.
```

The resulting conservative exception-count majorants are:

| Event | Count majorant before division by `|K|` |
| --- | ---: |
| No common row despite close mixture | `342 * (35260596266/3)` |
| Any incorrect-row mix cancellation | `524288` |
| Numerator cancellation at a violating subgroup point | `1` |
| Joint batch lacks a common polynomial triple | `2 * 4409294859` |
| FRI threshold crossing, including terminal | `8818455499/2` |
| **Total** | **`8065872632161/2`** |

The FRI sum uses next-domain lengths totaling 524,280, sixteen additive 11s,
and six terminal exceptions. All counts may be fractional majorants; actual
exception sets have integer cardinalities bounded above by them.

The repository checker `scripts/fastpq/check_compact_air_bound.py` checks these constants
and verifies the 237-query `<2^-128` inequality using integer/rational arithmetic.
It also verifies that this bound at 136 and 200 queries exceeds `2^-128` and
that 236 queries do not suffice for this particular bound. Arithmetic checks
are not a machine proof of the source theorem or this reduction.

## What this advances and what it does not

This draft removes separate oracle-proximity and extraction-chronology
premises from a proposed **formal AIR interactive** soundness argument. It
accounts for current/next consistency, independent column coefficients,
post-alpha quotient selection, the joint trace-degree shift, adaptive FRI
words, full terminal checking, and without-replacement query positions.

Separate internal mathematical review checked the coefficient-vector induction,
uniqueness before alpha, the whole-row cancellation event, and initializing FRI
with the actual passing-set measure. External review and review of the concrete
AIR degree/semantic ledger are still required. The prototype remains test-only;
237 is neither an implemented profile nor enough to cover unspecified qROM
losses. Byte limits, timings, authenticated caller integration, commitment and
transcript security, the compiler, concurrent attempts, and release evidence
must be addressed before any production parameter or activation decision.
