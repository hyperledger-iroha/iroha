# FASTPQ masked DEEP hiding construction

The normal-library offline quantity producer uses the single masked profile in
the [protocol contract](fastpq_deep_protocol_contract.md). This note gives its
current algebraic hiding argument and the remaining transcript obligations.
It does not select another profile or change admission limits. Native artifacts,
hardware scope and remaining integration checks belong in
[production readiness](fastpq_production_readiness.md).

## Current geometry and witness disclosure

The profile fixes `p=2^64-2^32+1`, `K=F_p[u]/(u^4-7)`, `N=65,536`,
`M=8,388,608`, and `q=77` distinct base-domain queries per child. There are
301 committed private columns and 41 unchanged public columns reconstructed by
the verifier. Multiplication by the trace generator `omega` advances a domain
index by 128. Each private source polynomial `C_j`, of degree below `N`, becomes

`A_j(X)=C_j(X)+(X^N-1)r_j(X)`, with `r_j` uniform in `F_p[X]_<162`.

For fixed independent honest-verifier coins, the required disclosure closure is

`S=Q_D union omega*Q_D union Orb_p(z) union Orb_p(omega*z)`.

The 77 next-row points are analytical coordinates needed to evaluate the AIR;
the proof directly opens only the 77 current rows. The closure has at most
`2q+8=162` distinct points, is Frobenius closed, and is disjoint from the trace
subgroup. Interpolation on this set has base-field coefficients, and `X^N-1`
is nonzero on every point. Evaluation of the 162 mask coefficients is therefore
onto the compatible answer space. Fresh uniform masks hide the witness on this
entire closure, including the next rows used by quotient evaluations.

The worst-case rank is attained by legal queries `0..76` and `z=u`: the
independent exact field model gives rank 162. Reducing the mask to 161
coefficients gives rank 161 and admits a distinguishing linear functional.
For the permitted quadratic challenge `z=u^2`, the rank is 158. Thus eight
OOD dimensions are a maximum, not the dimension of every sampled challenge.

## Quotient masking and exact degrees

The producer interpolates the complete 923-slot numerator on the `4N` domain
and divides exactly by `X^N-1`. Every remainder coefficient must vanish.
Pointwise agreement at selected evaluations would not establish this condition.
Split the quotient by coefficients and sample an independent polynomial
`T` uniformly in `K[X]_<78`:

`Q=Q0+X^N Q1=(Q0+X^N T)+X^N(Q1-T)=Q0'+X^N Q1'`.

On `U=Q_D union {z}`, at most 78 distinct points, the trace closure determines
the total quotient answers. Evaluation of `T` is onto the extension-field
answer space. The high-chunk answers are uniform; the low-chunk answers follow
from the displayed identity. This argument permits the unequal chunk degrees
used by the implementation.

| Polynomial | Exclusive degree bound |
| --- | ---: |
| Masked private trace | 65,698 |
| Complete mixed AIR numerator | 196,803 |
| Quotient after zero-remainder division | 131,267 |
| Randomized low chunk | 65,614 |
| Randomized high chunk | 65,731 |
| Independent composition mask and composition | 131,072 |

The high quotient chunk must not be truncated to `N` coefficients: the original
quotient can have 195 coefficients beyond `2N`. The native degree owner retains
the public-column bounds and the actual hash/SMT selector structure. Its
private-next affine regressions check the compiled numerator expressions; a
squared private-next mutation exceeds the affine bound.

## Composition and complete algebraic answer view

Sample `R` independently and uniformly from `V=K[X]_<2N` and commit it with
`Q0'` and `Q1'` before the OOD and batching challenges. The 606 DEEP relation
terms have distinct weights `lambda,lambda^2,...,lambda^606`; `R` alone has
weight one. Both shifted terms for each trace/quotient component remain in the
relation. Write their sum as `D_lambda`, so the composition is

`B=R+D_lambda`.

For an honest source, `D_lambda` belongs to `V`. Translation of uniform `R`
makes the entire polynomial `B` uniform in `V`, independently of the trace and
quotient polynomials. At an opened point, `R(x)=B(x)-D_lambda(x)`; the right side
uses the already simulated trace, quotient and OOD answers. All mixed-arity
FRI layers, fibers and the complete terminal are deterministic functions of `B`
and the verifier's folding coins. They add no witness dependence to this
algebraic answer view.

A witnessless algebraic simulator samples compatible trace answers, uniform
high-chunk answers and uniform `B`, derives low-chunk and mask answers, and
folds `B`. This establishes perfect independence of the honest-verifier
algebraic answer view under independent uniform masks and verifier coins.
It includes FRI disclosures but does not by itself simulate Merkle
authentication, extra oracle queries, adaptive Fiat–Shamir transcripts or
implementation side channels.

The underlying ideas follow Haböck–Al Kindi's
[*A note on adding zero-knowledge to STARKs*](https://eprint.iacr.org/2024/1037).
Their quotient split differs from this fixed `X^N` split; the interpolation and
translation arguments above provide the local adaptation. An imported theorem
for another split or FRI schedule cannot replace the remaining source-specific
transcript argument.

## Ownership, resources and remaining qualification

`deep_masked_replay`, `deep_masked_quotient` and `quotient_pair_masking` use
clearing owners, bounded canonical sampling and explicit entropy. Production
uses `OsRng`; there is no seeded fallback. Failed attempts do not reuse masks.
The producer retains coefficients, masks, an active stripe and bounded tree
replay/cache storage. It does not retain the complete private LDE. The shared
plan charges all admitted phases before private transforms, including the
retained Metal pool, and the wrapper separately admits source conversion,
private SMT, bundle and decode costs.

The unchanged limits are a 2 GiB construction payload charge per segment,
`2^42` structural work units, a 512 KiB child cap and a 1 MiB complete artifact
cap. The fixed child layout has a 500,084-byte upper envelope. These charged
payload/work bounds are distinct from measured process RSS and elapsed time.
The protocol contract owns exact framing and cumulative two-child verification
costs, including 154 queries for two children.

Complete qualification still requires the adaptive Merkle/Fiat–Shamir simulator
and its stated classical/quantum assumptions, concrete hash/XOF and RNG
assumptions, lifetime/multi-target accounting, and arithmetic/side-channel
evidence. Bounded algebraic checks and a conditional distinguishing-distance
calculation do not establish those claims. Mathematical truth of a bound
statement also does not authenticate source finality, permissions or replay
protection; Core owns those separate checks.
