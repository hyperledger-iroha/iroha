# Standalone election: late-dropout aggregate-opening blocker

This is a scoped F11 design review on the `optimizations` source as observed on
2026-09-23. It gives a necessary recovery interface and concrete rejection
tests; it is neither a general impossibility theorem nor a completed protocol,
security proof, circuit, key, or production qualification. The
[protocol contract](../../../specs/standalone_election_protocol_contract.md)
and [audit matrix](../../../specs/zk_audit_matrix.md#election-statement-completion)
remain controlling. The earlier [candidate review](../2026-09-21/standalone-election-candidates.md)
and [fresh-mask rejection](election-fresh-mask-correction-rejection.md) cover
specific constructions. No examined construction supplies the missing
aggregate-only opening below.

## Exact object to be opened

Let `P` be the frozen network, election, eligibility, asset incarnation and
scale, options, conviction formula, arithmetic limits, and deadlines. Let `H`
be the **finalized ordered accepted history**, including every ordinary ballot
and conviction update. `L(H)` selects only the latest accepted state for each
credential-linked election nullifier. For `K` options, the required result is

`T(H)[k] = sum_{x in L(H)} weight_P(x) * 1[choice(x) = k]`, for `0 <= k < K`.

Here `weight_P` is computed from the confidential, conserved bond in frozen
smallest asset units with checked integer square root and multiplier. Ordinary
ballots never change choice. An update must prove the same hidden choice,
nondecreasing amount, duration, and expiry, a strict amount/expiry increase,
and replacement of exactly its previous accepted weight. The result proof must
bind the actual `H`, latest-state map, count/root, `P`, and exactly one `T(H)`;
a submitter-selected ciphertext list or proof of the asserted totals alone is
insufficient.

## Necessary recovery interface, still without an implementation

A qualifying proposal must instantiate an authenticated `Accept(P, history
prefix, ballot or update, recovery material)` and one `Finish(P, H,
close certificate, durable material, public-worker messages) -> (T(H), proof)`.
The chain supplies finalized order and durable availability under its own
validator fault model. The proposal must separately choose a voter-dropout
bound `d >= 1`, an active-corruption bound, worker availability and synchrony
bounds, finite phase deadlines, and an abort rule. `Accept` is the durable
consensus transition, not a relayer acknowledgement. For every accepted message,
the voter may disappear immediately; `Finish` must still include its latest
weight without requesting its secret or deleting its ballot. A setup publisher
who never casts must not leave an uncancellable contribution.

Consequently, whatever enables completion for a dropped voter must be durable
by that voter's **last accepted** transition, or be derivable later by parties
whose guaranteed availability is included in the declared fault bound. The
opening capability must be restricted to the unique finalized `H`; it must not
also open a proper prefix, a different survivor set, an earlier version of an
updated ballot, or a tally submitter's chosen subset. This restriction must
hold against an adversary that observes all ledger and phase messages, actively
schedules updates and dropouts, and retries after lost replies or restarts.
Closure requires an authenticated finality certificate and exact derived root;
merely labelling a phase as closed does not enforce this cryptographic
restriction.

The capability has no established source under the fixed product boundary:

- Keeping a needed opening with the voter fails when that voter disappears
  immediately after acceptance.
- Publishing a generally usable per-ballot opening, mask correction, or
  aggregate opening for more than the one certified corpus creates an extra
  plaintext or subset-result channel. The algebra below gives a direct update
  distinguisher for additive masks.
- Having designated workers hold decryption shares or a master key would
  supply a decryptor/committee, which the contract forbids. An MPC proposal
  among voters or public workers cannot simply rename that role: it must state
  exactly what they hold, what a corrupt coalition can compute, why no voter
  secret is reconstructed, and why the workers are not a decryption committee.
  No such protocol and proof are supplied here.

This classification does **not** rule out a future publicly verifiable,
closed-corpus-specific functional opening from preaccepted material. It
identifies the primitive and composition that must be constructed and reviewed.
A generic homomorphic ciphertext, time-lock opening, or zero-knowledge tally
proof does not by itself provide the one-corpus restriction or an available
post-dropout witness.

## Concrete disclosure tests

For an additive group ballot, write the old and updated components for option
`k` as `C[k] = g^(w * 1[c = k] + r[k])` and
`C'[k] = g^(w' * 1[c = k] + r'[k])`, where `w' > w` and `c` is hidden.
If the public can obtain the per-update mask difference
`D[k] = g^(r[k] - r'[k])`, it computes
`C'[k] / C[k] * D[k] = g^((w' - w) * 1[c = k])`.
For a positive bounded increase below the group order, the identity/nonidentity
pattern identifies `c` without solving a discrete logarithm. Reusing the mask
is the case `D[k] = 1`; publishing a fresh-mask correction has the same failure.
This rejects **these public linear correction interfaces**, not all possible
functional-opening protocols.

Likewise, if `Finish` or another public operation returns both the exact total
for a pre-update corpus and the total for the same corpus with one accepted
update, their vector difference is `(w' - w) * e_c`. For an equal-final-output
privacy comparison, give voter A weight `1`, then `2`, and voter B weight `2`.
In one world A chooses option 0 and B option 1; in the other they exchange
choices. Both final vectors are `(2, 2)` and can have the same allowed traffic
metadata, yet the pre-update vectors and any isolated update difference reveal
A's option. A finality-gated opening must therefore prevent early and
counterfactual corpus results, including through retries and alternate
survivor sets. The final totals may themselves identify a choice when too few
honest voters remain; the privacy definition must compare worlds with equal
final totals, metadata, and corrupted inputs rather than promise otherwise.

## Phase and failure obligations for the next candidate

| Checkpoint | Required property and rejection trace |
| --- | --- |
| Freeze/enroll | Bind `P`, credential eligibility, asset/scale and `K = 2..=64`; a setup-only dropout must not change the exact active corpus or require disclosure of an active voter's choice. The relayer is transport and fee payer only. |
| Cast/update acceptance | One proof relates anonymous credential, election nullifier, confidential owned bond, conservation and a well-formed hidden option/weight. An update proves the prior accepted state and same choice. Acceptance includes all recovery material necessary if this voter is the next dropout; stale/concurrent updates and duplicate retries have one ordered outcome. |
| Close | Derive `H`, `L(H)`, count and root from finalized execution, with one immutable context and close certificate. Reject a selected subset, a stale root, changed policy, network, credential or phase, and a second close after restart. |
| Finish | For any adversarial dropout set within `d`, and every permitted worker-failure schedule, publish only `T(H)` and a sound relation to the closed corpus. Neither a prefix tally, an individual correction, a dropped secret, nor a changed survivor corpus may appear. |
| Persist/release | Store one immutable result and its evidence; an identical retry is idempotent. Unlocking or slashing a bond cannot erase the result relation or allow a second spend. |

No values for `d`, corruption threshold, worker count, rounds, or deadlines can
be qualified before a construction exists. A candidate also needs maximum
eligibility/history/update counts, exact aggregate and field bounds, proof and
message sizes, durable storage/I/O, worst-case proving and verifying memory/work,
network rounds, and public tally-extraction cost. In particular, a group tally
encoded as `g^T` requires a measured bounded discrete-log extraction for the
largest **smallest-unit** conviction total; a short validity proof does not
bound extraction time. Hardware-dependent acceleration may improve speed but
must not change the result.

## Scoped PLAIN smallest-unit arithmetic check

A separate static read of the existing PLAIN conviction path found no concrete
rounding or overflow defect to patch in the election data model. `Quantity`
canonicalizes trailing decimal zeroes and rejects negative values
([numeric owner](../../../crates/iroha_primitives/src/numeric.rs)). The frozen
scale conversion rejects a finer canonical scale, a mantissa outside `u128`,
and a checked `mantissa * 10^exponent` overflow; it performs no rounded
conversion. With accepted units at most `u128::MAX`, integer square root is at
most `u64::MAX`. The capped multiplier is at most `u64::MAX`, so even their
maximum product fits `u128`; the implementation still uses checked
multiplication. PLAIN category and turnout additions are checked, and the
approval comparison uses a full-width product rather than truncating a ratio.
Core checks the new bond and the exact `Quantity` delta before an authenticated
asset movement; its live asset-scale check separately rejects a transfer the
current asset specification cannot represent. Existing smallest-unit, square-root,
maximum-value and aggregate tests were read, not rerun during the shared Cargo
build. This is a bounded source audit, not private-election qualification.

The separate PLAIN approval policy did admit a zero numerator, so an all-Nay
tally could satisfy its inclusive zero threshold. `PlainConvictionPolicyV1`
now rejects that policy at validation and decision time. The fresh compiled
`governance::conviction::tests::` selector passed 8/8, including the Nay-only
regression and the existing smallest-unit/overflow cases. This does not open
the standalone private ballot route.

## Source boundary and decision

The current source has no such interface. `SubmitBallot` supplies opaque bytes,
a proof attachment and a nullifier; `FinalizeElection` supplies `Vec<u64>`
asserted totals and a proof attachment
([data-model instructions](../../../crates/iroha_data_model/src/isi/zk.rs)).
Core currently reads only commitment/root ballot columns and one `u64` per
option, derives a nullifier from the public commitment, and checks that the
supplied ciphertext equals that commitment. Its explicit semantic guard rejects
the standalone ZK ballot and tally routes before accepted mutation or final
proof verification
([Core owner](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)).
The current election state retains nullifiers and a bounded ciphertext vector,
not the latest accepted confidential bond/choice-preserving update relation
([state shape](../../../crates/iroha_core/src/state.rs)). The public PLAIN
conviction arithmetic uses `u128` checked weights and a frozen scale
([arithmetic owner](../../../crates/iroha_data_model/src/governance/conviction.rs));
it is not an anonymous tally proof.

**F11 remains unresolved.** The next admissible deliverable is a specified
closed-corpus functional-opening protocol with its fault thresholds, active
privacy definition, availability and soundness arguments, arithmetic/resource
bounds, and independent review. This record authorizes no circuit ID, key,
fallback, committee, or production route.
