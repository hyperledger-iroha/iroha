# Standalone election proof qualification

The production standalone election registry currently admits no semantic ballot
or tally circuit. `smartcontracts/isi/world.rs::voting_circuit_matches` requires
the closed Halo2 registry; `zk.rs::validate_builtin_halo2_ipa_verifying_key_v1`
has no ballot/tally implementation. The test-only depth-8 Boolean Merkle circuit
and tiny-add circuit prove different relations and cannot satisfy this boundary.
Generic public-binding STARK proofs are also rejected for these roles.

The maintained integration tests execute normal production-library rejection
and IVM readback behavior without a runtime environment skip:

- `gov_zk_ballot_real_vk`: unsupported roles cannot register; election creation
  rejects missing keys and corrupt retained records without committing state.
- `gov_zk_ballot_lock_verified`: rejected create/extend/shrink attempts leave the
  exact retained lock, referendum, nullifier set and ciphertext corpus unchanged;
  only the exact rejection event is emitted.
- `gov_finalize_real_vk`: unsupported tally roles cannot mark a retained election
  finalized or replace its tally, nullifiers or ciphertexts.
- `zk_vote_get_tally`: the actual IVM syscall returns the finalized `[4, 0]` snapshot,
  including after an invalid replacement was rejected. This tests typed state
  readback, not cryptographic tally verification or consensus finality.

These negatives do not discharge the original positive requirements.

TODO: implement and independently qualify the distinct semantic ballot/tally
relations, exact verifier keys, public-input bindings, and governed admission.
Then execute the following source-bound production-library acceptance sequence:

1. Register the exact ballot and tally keys through the normal registry and
   create the election through `CreateElection` with the current role metadata.
2. Open the standalone referendum through its actual Parliament gate; a fixture
   whose status remains `Proposed` must not accept a ballot.
3. Verify a real ledger-bound ballot that locks 1000 for 200 blocks, asserting
   the exact owner, one `LockCreated` event, one consumed nullifier and one
   retained ciphertext.
4. Verify a fresh ballot opening that extends the lock to 1200 for 400 blocks,
   asserting `LockExtended` and the second distinct consumed nullifier. Never
   erase the first nullifier to obtain an accepted re-vote.
5. Submit an independently valid proof requesting 900 for 250 blocks; assert
   the actual `re-vote cannot reduce existing lock` error and matching
   `BallotRejected` event, with no lock/corpus/nullifier mutation.
6. Verify a semantic tally proof whose values are exactly submitted, then assert
   `finalized` and the full tally. Keep the original first value 4. The generic
   snapshot shape permits `(options=1,tally=[4])`; current standalone referendum
   decision persistence separately requires at least two tally slots. Resolve
   that intended consumer shape explicitly with its owner when qualifying the
   positive, without changing the readback DTO or substituting tiny arithmetic.

The current negative fixtures directly seed unadmitted retained records solely
to exercise fail-closed production consumers. They are not live provenance,
valid proofs, accepted elections, or four-validator deployment evidence.
