# Standalone election final-corpus opening audit

This is a bounded F11 design audit on `optimizations`, 2026-09-24. It identifies
the remaining cryptographic interface; it selects no protocol or verifying key,
proves no general impossibility, and supplies no production qualification. The
[standalone election contract](../../../specs/standalone_election_protocol_contract.md)
controls the required behavior.

## Current source and fault boundary

[`CreateElection`, `SubmitBallot`, and `FinalizeElection`](../../../crates/iroha_data_model/src/isi/zk.rs)
still carry an eligibility root, opaque ciphertext and proof, caller-supplied
nullifier, and asserted totals. **Current** `FinalizeElection.tally` and
[`ElectionState.tally`](../../../crates/iroha_core/src/state.rs) are `Vec<u128>`;
older review prose describing `Vec<u64>` is stale after the
[representation cut](standalone-election-u128-tally-representation.md). Core
[`SubmitBallot`](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)
still checks transaction-authority permission and citizenship, derives the
nullifier from a public commitment, and retains a nullifier set and bounded
ciphertext vector rather than a credential-linked latest-state and confidential
bond history. The
[`ensure_qualified_standalone_zk_relation_v1` guard](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)
rejects ballot and tally execution before proof verification or accepted-state
mutation; the [production circuit registry](../../../crates/iroha_core/src/zk.rs)
contains no governance ballot or tally circuit. The separate public
[conviction arithmetic](../../../crates/iroha_data_model/src/governance/conviction.rs)
uses checked smallest-unit `u128` weights, but does not authenticate a private
bond or choice.

Let `P` freeze network, election, eligibility, asset incarnation and scale,
`K = 2..=64` options, exact weight rule, limits, and deadlines. Let `H` be the
one finalized ordered history; `L(H)` retains the latest accepted cast/update
for each credential-linked nullifier. The sole permitted result is
`T(H)[k] = sum_{x in L(H)} weight_P(x) * 1[choice(x) = k]`. A candidate must
fix maximum eligible credentials `N`, accepted operations `A`, updates per
credential `U`, voter dropouts `d >= 1`, active corruptions `c`, worker and
network availability, and a finite close-to-result deadline. Chain finality
does not supply voter availability. After **every** finalized cast or update,
the voter may stop permanently; setup-only noncasters contribute nothing.
Privacy compares complete public views with equal final `T(H)`, allowed
metadata, and corrupted inputs, including adversarial schedules and retries.
It forbids individual plaintexts and any second prefix or proper-subset tally.

For additive masked ballots, `C_i[k] = g^(w_i * 1[b_i = k] + r_i[k])`.
If an update publishes `C'_i` plus a public correction
`D_i[k] = g^(r_i[k] - r'_i[k])`, anyone computes
`C'_i[k] / C_i[k] * D_i[k] = g^((w'_i - w_i) * 1[b_i = k])`.
For a positive bounded increase, the identity pattern reveals the hidden
choice without a discrete logarithm. Likewise, two valid results for histories
differing by one update reveal that choice by subtraction. For example, one
voter's weight changes `1 -> 2` and another's weight is `2`; swapping their
choices preserves final `(2, 2)` while changing the pre-update totals. A
single-update batch is already a distinguishing test. Generic time release of
individual masks or a general homomorphic decryption key also permits
singleton opening after release. A public validity check on the requested
corpus cannot cryptographically restrict reuse of those released values.

## Primary-source candidate checks

| Candidate | Precise unmet condition here |
| --- | --- |
| [Decentralized MCFE, Chotard et al. (ASIACRYPT 2018)](https://www.iacr.org/archive/asiacrypt2018/11272256/11272256.pdf), Definition 4 | There is no master secret, but the eventual function key combines partial keys from the included clients. An accepted voter disappearing before issuing its final-corpus function-key share can block completion. Preissuing keys for alternate histories needs a separate proof that they cannot open a prefix or subset. |
| [Ad hoc MIFE, Agrawal et al. (ITCS 2020)](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ITCS.2020.40) | Dynamic selection still requires a separate function-specific key from each selected source. Its dynamic subset feature does not enforce this election's unique finalized `L(H)`. |
| [Robust DMCFE, Li et al. (ASIACRYPT 2023 slides)](https://iacr.org/submit/files/slides/2023/asiacrypt/asiacrypt2023/55/slides.pdf) and [flexible-threshold MCFE, Zhang et al. (2025)](https://arxiv.org/abs/2510.15367) | Their dropout interface computes over available/positive clients (or substitutes a default for unavailable clients) when a threshold remains online. It does not establish inclusion of a previously accepted client that disappears before the final function-key message. A key holder and any threshold shares also need the contract's no-decryptor analysis. |
| [NARAD, Venkata et al. (2026)](https://arxiv.org/html/2607.07596v1), Sections 6 and 8 | A durable ciphertext/auxiliary pair can survive its voter, but the collector and keyed aggregator are privileged. With per-voter auxiliaries, their coalition can multiply any selected pairs and open a proper subset, including a singleton; the paper's non-collusion condition is outside this contract. See the [subset-opening derivation](standalone-election-narad-subset-opening-review.md). |

These are failures against this **combination** of requirements, not claims
that the papers' own theorems fail or that every possible cryptographic
construction is impossible. In particular, a genuinely one-corpus functional
opening is not ruled out. No reviewed source here supplies it with dynamic
latest-state selection, atomic post-accept recovery material, and the required
privacy game.

## Next reviewable cut

Specify an `Accept(P, prefix, cast/update, proof, recovery_artifact)` transition
that makes its entire recovery artifact durable atomically with consensus
acceptance. Its proof must tie the anonymous credential, election nullifier,
confidential conserved bond, frozen smallest-unit weight, valid hidden choice,
and, for an update, the same choice and exact previous accepted state. Specify
`Close(P, H) -> (finality_certificate, root, count, latest_state_commitment)`
from actual finalized history, and a public-worker
`Finish(P, H, certificate, durable_artifacts) -> (T(H), proof)` whose output
cannot be produced for any other corpus, prefix, or subset. The missing
deliverable is the **concrete cryptographic algorithm and reduction** for that
last restriction: all accepted-voter material must be present before dropout,
no worker may hold a general opening key, no voter secret is reconstructed,
and a valid proof must bind every latest ballot to the certified corpus.
Publishing these phase names or a proof of asserted totals alone does not fill
the opening interface.

Set resource bounds before implementation. Current Core limits retained
ciphertexts to `ballot_history_cap.clamp(1, 1000)` while the configured default is
1024; neither is a reviewed bound on `N`, `A`, or `U`. At `A = 1000`, `K = 64`,
even one 32-byte group element per operation and option is 2,048,000 bytes
before proofs and recovery data. Pairwise per-option material for `N = 1000`
would be 1,022,976,000 bytes at 32 bytes per element. A `u128` exponent-encoded
sum cannot be assumed cheaply extractable: generic square-root discrete-log
work over its full range is on the order of `2^64` group operations. The next
candidate must measure worst-case storage, wire, proving/verifying work, tally
extraction, restarts, and deadline cost at declared `N/A/U/K/d/c` maxima.

No code or proof was changed by this audit. F11 and production admission remain
closed.
