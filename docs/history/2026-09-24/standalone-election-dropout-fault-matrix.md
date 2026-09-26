# Standalone election dropout fault and acceptance matrix

This is a scoped F11 design and source review on `optimizations` on
2026-09-24. It adds quantified obligations to the
[standalone election contract](../../../specs/standalone_election_protocol_contract.md)
without selecting a cryptographic construction or opening the production
route. The [aggregate-opening blocker](../2026-09-23/standalone-election-dropout-aggregate-blocker.md)
and [fresh-mask rejection](../2026-09-23/election-fresh-mask-correction-rejection.md)
remain applicable.

## Source boundary

`CreateElection` currently has an eligibility root, deadlines, option count,
two verifying-key IDs and a nullifier domain, but no frozen confidential bond
asset, scale, conviction policy or phase protocol. `SubmitBallot` carries opaque
ciphertext bytes, a proof attachment and caller-supplied nullifier;
`FinalizeElection` carries asserted `Vec<u64>` totals and a proof attachment
([instruction shapes](../../../crates/iroha_data_model/src/isi/zk.rs)). Core
derives the nullifier from the public commitment and stores a nullifier set plus
bounded ciphertext vector, not a credential-linked latest-state map with
confidential bonds ([execution](../../../crates/iroha_core/src/smartcontracts/isi/world.rs),
[state](../../../crates/iroha_core/src/state.rs)). The explicit Core semantic
guard still rejects standalone ZK ballot and tally execution before proof
verification and accepted-state mutation. This is the correct admission state
for the current relation.

The separate public PLAIN conviction path freezes asset scale and computes
`floor(sqrt(exact smallest units)) * min(1 + duration / step, cap)` with checked
`u128` weights and checked aggregate arithmetic
([arithmetic](../../../crates/iroha_data_model/src/governance/conviction.rs)).
Its passing focused test is evidence for public arithmetic only. In
particular, the current ZK `Vec<u64>` result shape cannot be presumed to
represent the maximum `u128` conviction aggregate. The final private statement
and Norito interface must establish and encode an explicit compatible bound;
retaining an overflowing or truncating `u64` route is not acceptable.

## Fault domains a candidate must fix

The chain's signed finality and durable data availability do not imply a voter
will send another message. Let `N` bound eligible credentials, `A` bound the
accepted ordered cast/update history, `U` bound updates per credential,
`K` be 2..=64 options, `d >= 1` be tolerated voter disappearances and `c`
bound active corrupt credentials. A candidate must assign concrete values,
units and relationships, plus worker availability, synchrony and deadline
bounds. It must name whether corruption can occur after casting and precisely
exclude only a corrupted voter's own disclosed input from honest-ballot
privacy. A relayer's signature or fee payment is never ballot authorization.

The acceptance boundary is the hard case: after **each** finalized cast or
update the voter can disappear immediately. All material required for a
correct final tally must already be durable or available from the declared
fault-tolerant public process, without reconstructing any voter secret.
The final result is over the one closed finalized history, not a survivor
subset. Exceeding `d` may leave liveness unresolved but cannot turn a partial
or early result into a valid one. A setup-only dropout cannot contribute a
phantom ballot or block a closed corpus that contains only actual accepts.

The phase and adversarial cells in the contract require adversarially timed
cast/update dropouts, active invalid messages, concurrent and replayed updates,
one-update batches, close/finish retries, and restart at each durable step.
For privacy, compare complete views of executions with the same final totals,
allowed public metadata and corrupt inputs; compare traffic and intermediate
outputs, not only final proofs. One final tally may inherently disclose a
choice when few honest voters remain, but an additional prefix, counterfactual
or subset tally would exceed this contract. For correctness, only the latest
accepted version of each credential-linked nullifier contributes; an ordinary
ballot is immutable and an update preserves its hidden choice while increasing
its amount or expiry under the nondecreasing lock constraints.

## Open construction and release gates

There is no concrete closed-corpus-only functional opening, fault threshold,
active privacy reduction, sound tally relation, credential/bond circuit,
phase transcript or measured maximum proof/tally cost in this review. An
unprivileged tally submitter or public worker may make progress, but no worker
or coalition may become a decryption committee or obtain master/voter secrets.
Any private worker state needs an explicit non-decryptor explanation and a
validity/availability proof. An additive fresh-mask correction fails even when
final totals match, as the prior rejection trace demonstrates; replacing it
with a different name or batching a single update does not close that leak.

The next candidate must instantiate the quantified model, prove exact
accepted-corpus inclusion and aggregate-only disclosure, then supply typed
Norito state/messages/circuits and maximum-shape resource measurements for
independent review. F11 and the standalone production guard remain open.

Validation: static source and document inspection only; no Cargo or protocol
qualification was run for this documentation change.
