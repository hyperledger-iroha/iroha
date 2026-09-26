# Standalone election: NARAD aggregate-opening review

Read-only F11 research on `optimizations`, 2026-09-24. This is a precise
candidate rejection against the [standalone election contract](../../../specs/standalone_election_protocol_contract.md),
not an impossibility theorem, selected construction, circuit, or production
qualification. The earlier [functional-opening review](standalone-election-primary-source-functional-opening-review.md)
remains the broader comparison.

## Candidate and exact disclosure test

[NARAD (Venkata, Dugar and Adarsh, July 2026)](https://arxiv.org/html/2607.07596v1)
has each voter publish a masked ciphertext
`c_i = H^s_i (1+N)^x_i mod N²` and give a collector
`aux_i = H^(sk_A s_i) mod N²`. Its aggregator, which holds `sk_A`,
multiplies ciphertexts, cancels the product of auxiliary values and recovers
the packed sum (Sections 6.3–6.5, Equations 3–10). For a voter whose **entire**
pair is durably available, this algebra needs no subsequent voter message.
That is a useful post-cast-dropout property, conditional on collector and
aggregator availability and on both artifacts surviving acceptance.

**Inference from the same equations:** for any chosen subset `S`, the
collector can form `A_S = product(aux_i for i in S)`. With `sk_A`, the
aggregator can compute

`L(((product(c_i for i in S))^sk_A / A_S) mod N²) / sk_A mod N
 = sum(x_i for i in S) mod N`.

For `S = {i}` this opens an individual ballot. A collector–aggregator
coalition therefore has a proper-subset and singleton tally oracle, even if
the honest run publishes only one final result. This is outside the paper's
explicit non-collusion model (Sections 4.2 and 8.4) and outside the product's
aggregate-only disclosure boundary. The paper's implementation description
also says **both** `c_i` and `aux_i` are stored on Solana (Section 11.4),
while its aggregator privacy argument assumes the aggregator never sees an
individual `aux_i` (Proposition 8.3). If those on-chain values are public as
described, the holder of `sk_A` alone can run the singleton equation. This is
a protocol/implementation-description conflict to resolve in any independent
review, not a claim that this repository runs NARAD.

The paper's protocol section instead sends `c_i` to the chain and `aux_i` to
the collector (Section 6.3). **Inference:** if acceptance makes only `c_i`
durable, a voter disappearing before durable `aux_i` delivery leaves its
accepted ballot uncountable. Making both artifacts atomic at acceptance
repairs that availability edge, but does not remove the subset-opening power.
Revealing only the collector's final product makes that collector an
availability and exact-corpus authority; the paper itself states that the
single aggregator can withhold a result (Section 8.4). A committee or enclave
substitution would change the trust model and is not an allowed recovery path.

The paper assumes binary per-option inputs, does not integrate ballot range
proofs, and has no anonymous credential, confidential bond, smallest-unit
conviction rule, choice-preserving update or finalized latest-state proof
(Sections 4.2 and 8.4). Selecting latest ciphertext pairs after updates would
still leave the collector able to construct earlier or alternative subset
products. The reference `k=10`, `b=25`, 255-bit modulus is a proof-of-concept
capacity point, not this release's audited security or maximum shape. Its
packing condition is `k*b <= floor(log2 N)` (Assumption 6.3): **conditional
inference:** if all 64 options may each reach a full `u128` aggregate, then
`b=128`, `k*b=8192`, so `N` must be at least `2^8192` before any security
margin and ciphertexts are roughly 2 KiB each. A smaller audited election
bound may reduce that requirement; no production size/time measurement follows
from the paper's 255-bit benchmark.

[Dynamic decentralized functional encryption (Nguyen, Pointcheval and
Schädlich, 2025)](https://eprint.iacr.org/2025/290) removes a trusted third
party and permits dynamic joins, but its abstract expressly retains every
participant's contribution to a function key. **Inference:** it does not
eliminate the accepted-voter dropout/key-share question in the earlier review.
[Information-theoretic secure aggregation with user dropouts (Zhao and Sun,
2021)](https://arxiv.org/abs/2101.07750) recovers the sum of surviving
responders; **inference:** that is a different accepted-corpus rule and cannot
discard a finalized ballot here. Neither source provides the missing
one-finalized-corpus-only public opening.

## Contract consequence

No reviewed construction yet meets the conjunction of accepted-ballot
dropout, setup-only noncasting, latest conviction update, anonymous
credential/bond authorization, absence of a decryptor or committee, and no
extra subset total. A reviewable candidate still needs an explicit `N/A/U/K`,
`d >= 1` and corruption/worker/synchrony bound, an exact final-corpus opening
argument, equal-leakage-world privacy proof including the singleton subset
test, sound validity/update circuits and maximum-shape measurements. The
current standalone ballot/tally admission remains fail-closed; this review
adds no wire type, key, proof or compatibility path.
