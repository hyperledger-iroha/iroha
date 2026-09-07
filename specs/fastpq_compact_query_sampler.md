# Conditional bounded query-sampler lemma

Source reviewed 2026-09-06: [`sample_queries` and `sample_queries_from`](../crates/fastpq_prover/src/backend.rs), with maximum desired count 512, minimum 64 digest draws, eight draws per desired index, and six lanes per draw. This closes only a distribution/abort calculation **under the ideal assumption below**. It neither instantiates that assumption with Digest384 nor proves adaptive Fiat–Shamir/qROM security.

## Assumption and exact model

Fix a valid input and the pre-sampling transcript independently of the future draw outputs. Assume every requested digest is a fresh, independent, uniform member of `F_p^6`, where `p=2^64-2^32+1`. Thus all scalar lanes, both within and across digests, are iid uniform in `{0,...,p-1}`. This assumption is stronger than each lane merely having a uniform marginal distribution. No adversary has selected this attempt after observing its future outputs.

For a nonempty invocation, let `D` be the representable domain size, `m=min(target,D)`, with `1<=D<=p` and `1<=m<=512`. Define:

```
k = floor(p/D),  r = p-kD
B = max(64,8m) digest draws,  C = 6B scalar candidates
```

The code rejects candidates at least `p-r`; every remaining label `i` has exactly `k` preimages. A scalar therefore produces rejection with probability `r/p` and each label with probability `k/p`. This includes `D=p` (`k=1,r=0`). Empty domain or target returns the empty sample without drawing; unsupported nonempty inputs fail before drawing.

## Uniformity and stopping

**Lemma.** With the full `B` draws available, let `O` be the order in which the first `m` distinct labels are discovered and `T` the number of scalar candidates required. `O` is uniform among the `(D)_m` distinct ordered samples and is independent of `T`. Consequently, conditioned on successful stopping `T<=C`, the returned ascending vector is uniform among the `binom(D,m)` subsets of size `m`.

The returned ascending vector is not itself a uniform permutation: only the conceptual discovery order has that property.

**Proof.** After discovering `j` labels, the probability of a new one is `s_j=k(D-j)/p`. For any specified unseen next label and a wait of `g>=1` candidates, the probability is

```
(1-s_j)^(g-1) * k/p
  = [s_j*(1-s_j)^(g-1)] / (D-j).
```

Multiplying over `j=0,...,m-1` factors the joint probability into `1/(D)_m` and the product of geometric waiting-time probabilities. This proves independence of the complete waiting vector and discovery order, hence independence of `T`, success, and the consumed digest count `ceil(T/6)`. Sorting identifies exactly `m!` equally likely orders with each subset.

The implementation consumes a whole digest even when it stops partway through its lanes. Sampling its unused lanes in advance does not change any decision. It therefore succeeds exactly when `T<=6B`, including discovery in the sixth lane of the final draw. The full successful transcript advances by `ceil(T/6)` digest calls; no extra draw is needed. This distribution statement does not claim that the final digest/state is independent of the chosen set.

## Exact failure expression and rational upper bounds

An independent exact occupancy calculation uses integer masses `n[t,j]` for unfinished states with `j<m` labels, with `n[0,0]=1` and absent indices zero:

```
n[t+1,j] = (r+kj)*n[t,j] + k*(D-j+1)*n[t,j-1]
Pr[abort] = sum(j=0..m-1, n[C,j]) / p^C.
```

For efficient conservative certificates, set `b=(r+k(m-1))/p` and `R=C-m+1`. Before completion, the conditional probability of a non-new candidate is at most `b`. Failure requires at least `R` such positions. A union bound over their positions, applying the conditional bound successively, gives

```
Pr[abort] <= min(1, binom(C,m-1)*b^R).                 (1)
```

For this argument define a non-new indicator as zero after completion. This keeps the conditional bound valid and leaves every failed path unchanged; no independence of these indicators is assumed.

**All supported domains.** The following split proves a uniform bound below `2^-60`:

- If `D>=2m`, acceptance probability `kD/p>=1/2` and, before completion, at least half of accepted labels are new. Thus `b<=3/4`. Exact integer checks of (1) for each `m=1,...,512` give a bound below `2^-108`; the weakest certified exponent occurs at `m=8`.
- If `m<=D<2m`, then `r<=1022` and acceptance is at least `a0=1-2^-53`. At every stage `j`, its fresh-label probability is at least `a0*(m-j)/m`. Coupling geometric waits to collecting all `m` coupons with acceptance `a0`, followed by a union bound over missing coupons, gives `Pr[abort]<=m*(1-a0/m)^C`.

The latter bound needs no numerical exponential approximation. The binomial series implies

```
(1-a0/m)^(-m) >= sum(j=0..6, a0^j/j!) > 125/46.
```

Since `C>=48m`, this gives `Pr[abort] < 512*(46/125)^48 < 2^-60`. The script checks both strict rational inequalities exactly.

**Large-domain cases.** Equation (1), using only dyadic upper bounds on `b`, certifies:

| Domain `D` | Desired `m` | Draws `B` | Candidate count `C` | Certified abort bound |
| ---: | ---: | ---: | ---: | ---: |
| 524,288 | 136 | 1,088 | 6,528 | `<2^-69,379` |
| 524,288 | 512 | 4,096 | 24,576 | `<2^-237,070` |
| `p` | 512 | 4,096 | 24,576 | `<2^-1,319,995` |

For the first two rows `r=1`, and the script verifies `b<=2^-11` and `b<=2^-10`, respectively. These are loose upper bounds under iid sampling, not security bits of the proof or hash. A universal `2^-128` abort claim would be false: for `D=m=512`, a two-term Bonferroni missing-coupon bound certifies `2^-61 < Pr[abort] < 2^-60`. Specifically, singles have probability at least `(511/512)^24576`, and pairs at most `(46/125)^96`, so

```
Pr[abort] >= 512*(511/512)^24576 - binom(512,2)*(46/125)^96 > 2^-61.
```

Abort is an availability/completeness event, not false-statement acceptance.

## Transcript-counter capacity and limitations

For fixed starting transcript counter `c0`, only `A=u64::MAX-c0` calls can complete. Replace the cutoff by `C_eff=6*min(B,A)`. Uniformity conditioned on success still holds with this fixed cutoff, but the displayed abort bounds require `A>=B`.

At `c0=MAX`, a nonempty invocation fails before hashing. A draw starting at `MAX-1` may complete successfully and leave the counter at `MAX`. If the sample remains incomplete and `A<B`, the next requested draw returns the counter-exhaustion error. If `A=B`, reaching the loop end returns the draw-budget error instead. Empty-input behavior is unchanged. Operational counter exhaustion is not a rejection/duplicate event, and no random model for `c0` is assumed. The local draw tag remains below 4,096, so its `u32` index cannot overflow.

Canonical values alone do not imply the lemma. A fixed all-zero source can always fail for `m>1`. Even uniform marginal lanes are insufficient: over `F_5`, the digest `(U,U+1,...,U+5)` modulo five, for uniform `U`, produces only five of twenty discovery pairs and five of ten two-element subsets. Likewise, selecting among otherwise ideal attempts until a set contains a preferred index biases the selected attempt. Prior oracle queries, malicious commitment selection, concurrent proofs, concrete sponge correlations and qROM reprogramming are outside this lemma.

## Reproduction

Run `python3 scripts/fastpq/check_query_sampler_lemma.py` from the repository. The standard-library-only [script](../scripts/fastpq/check_query_sampler_lemma.py) checks live constants, all 512 count certificates, the three large-domain bounds and the dense-domain lower bound with integer/rational arithmetic. It also exhaustively enumerates small six-lane toy oracles, compares an independent occupancy recurrence, checks equal order counts at every consumed-digest count, and confirms the correlated-lane counterexample. There is no Monte Carlo simulation, Rust build, floating-point certificate or concrete-hash security test.

Validation on 2026-09-06: all checks passed, including an independent mathematical review and independent script run by the FRI agent. This conditional result can be used as one lemma in the remaining [soundness reduction work](fastpq_compact_soundness_sources.md); it does not close the compiler or aggregate security obligations.
