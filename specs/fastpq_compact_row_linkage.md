# Conditional row-linkage and constraint reduction

This is a deterministic/ideal-challenge sublemma for the exact compact contract. It does not prove that the committed words have the proximity properties assumed below, or that concrete transcript challenges are uniform. It is not production qualification.

Let F=F_p, K=F_(p^4), H=<g^8> of size N, D=a<g> of size L=8N, and omega=g^8. The points of D lie in F, D is disjoint from H, and multiplication by omega permutes D. Fix trusted numerator polynomials C_1,...,C_m with the implemented public selectors and statement. For any degree-<N column polynomials P=(P_1,...,P_w), assume each substituted C_k(X,P(X),P(omega X)) has degree <3N. This is a required property of the concrete AIR, not guaranteed by its Rust trait.

## Base-field recovery

If a polynomial P_j in K[X] of degree <N agrees with the committed F-valued column at at least N distinct points of D, then every coefficient of P_j lies in F. Apply the p-power map to its coefficients, leaving the indeterminate unchanged. At the agreement points, P_j^(p)(x)=P_j(x)^p=P_j(x), because x and the observed value belong to F. Their difference has degree <N and at least N roots, so it is zero. Each coefficient is fixed by the p-power map and hence belongs to F. This is conditional recovery of each polynomial; it does not produce a common agreement set for all columns.

## Current/next linkage and rejection set

Assume the complete committed row A(x) equals the vector P(x) on a common set G subset D, |G|>=sL. Let a committed quotient word Q agree with a degree-<2N polynomial q on G_Q subset D, |G_Q|>=tL. No independence of these sets is assumed. Then

    E = G intersection omega^(-1)G intersection G_Q
    |E| >= |G| + |omega^(-1)G| + |G_Q| - 2L
          >= (2s+t-2)L.

The shift permutation is exactly the verifier's current/next row map i -> (i+8) mod L; it is not an independently sampled row. At x in E, the verifier's quotient equality is equivalent to R_alpha(x)=0, where

    R_alpha(X) = sum_k alpha_k C_k(X,P(X),P(omega X)) - (X^N-1)q(X).

The division is safe because D and H are disjoint. If R_alpha is nonzero, its degree is at most 3N-1, so at least

    b = max(0, |E|-(3N-1))

positions in D fail the actual quotient check. The real failed-position count can be larger. Thus common row agreement and quotient agreement imply a useful query bound only when this intersection exceeds the degree budget. Pointwise closeness of each separate column, without a common set, does not supply this premise.

## Post-commitment numerator batching

Now additionally fix the statement, C and a violating P before drawing independent uniform alpha_1,...,alpha_m in K. This probability must not be conditioned on a later alpha-dependent proximity event. If P violates the AIR at some h in H, choose one such h before alpha. The vector (C_k(h,P(h),P(omega h)))_k is nonzero. Its inner product with alpha is zero with probability exactly 1/|K|. Therefore

    Pr_alpha[there exists degree-<2N q with R_alpha identically zero]
        <= 1/|K|.

The quotient q may be chosen adaptively after alpha: an identity R_alpha=0 would force that same inner product to vanish at h, since h^N-1=0. This argument does not incur a union bound over the m numerator slots. It fails as stated if the recovered P is selected after seeing alpha; the protocol-level extraction/correlated-agreement argument must supply the chronology. Satisfaction of these polynomial numerators still has to imply the intended transfer relation under the exact public-input and padding rules.

## Uniqueness at a sufficiently high common agreement threshold

If every candidate degree-<N row vector agrees with the same binding row oracle on at least sL positions and 2sL-L>=N, there is at most one candidate: the two agreement sets would intersect in at least N points, forcing equality of every degree-<N column polynomial. At L=8N this threshold is s>=9/16. Consequently, a threshold fixed before alpha can define a unique close P from the committed row oracle alone even if its existence is established later. The useful rejection-set regime above is stronger than this threshold when t<=1. This removes adaptive choice of P conditionally; it does not establish common proximity, efficient extraction, commitment binding or the required round-by-round security predicate.

## Query probability, conditional on the pre-query commitment prefix

Here the prefix contains binding commitments to fixed oracles and ends before query derivation. It excludes the query digests and opening answers. Choose the polynomials and agreement sets without the query randomness. For a fixed failed-position set B subset D with |B|=b, if the final query set, conditioned on sampler success, is a uniform q-sized subset of D, its probability of missing B is

    choose(L-b,q) / choose(L,q) <= (1-b/L)^q,

with zero probability if L-b<q. The equality needs the actual uniform-without-replacement distribution, and the right side follows by multiplying the q avoidance factors. This is a single conditional bad-set statement. Raising an unconditional one-query FRI error to q does not follow, because commit-phase bad events are shared across queries. B must be fixed before the query randomness. The concrete bounded sampler, ideal-oracle conditioning and qROM compiler remain distinct obligations.

The missing overall reduction must establish the common row/quotient proximity sets, valid extraction chronology, joint mixed/shifted-trace linkage, FRI commit-phase errors and the concrete transcript/hash assumptions. This lemma supplies none of those premises and assigns no security bits to the current profile.

Independent source/mathematical review found no algebraic defect under these premises. The [primary-source map](fastpq_compact_soundness_sources.md) records the remaining proximity/compiler obligations, and the [conditional sampler lemma](fastpq_compact_query_sampler.md) proves the required subset distribution only under iid, unselected ideal-oracle draws. No complete FASTPQ security bound is inferred.
