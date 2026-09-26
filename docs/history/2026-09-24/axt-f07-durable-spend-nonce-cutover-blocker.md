# F07 AXT durable spend-nonce cutover blocker, 2026-09-24

This is a source audit of the current `optimizations` checkout. It makes no
production admission change and adds no nonce ledger. The existing host and
block refusal of unanchored remote spending remains mandatory.

`AxtAnchoredSpendV1` already signs the canonical handle, intent, proof digest,
amount mirror, finalized-anchor fields, expiry, and nonzero fresh nonce. Its
`replay_key_v1()` identifies the complete issuer context plus nonce. These
DataModel checks are structural and cryptographic signature checks; the
submitted anchor is not resolved to independently finalized State/Kura data,
and the proof's successful ordered transfer, full WSV roots, amount, and
source transaction membership are not authenticated there. The existing
adversarial model test mutates the intent, proof, amount, anchor, and nonce.

The application owner cannot yet consume this key. `AxtEnvelopeRecord.handles`
still stores `Vec<AxtHandleFragment>`, with no signed spend or nonce. CoreHost
materializes that old envelope and rejects remote handle use before successful
completion. Block validation independently rejects any envelope with handles.
`StateTransaction::record_axt_envelope` stages handle-budget and permanent
counter changes from those fragments, then records the old replay entries;
the latter can expire and do not represent fresh issuer spend nonces.

Consequently, a new World nonce map or a test-only insertion into
`record_axt_envelope` would not consume an authoritative signed spend.
Deriving a nonce from a handle sub-nonce would change its identity. Inserting
on a submitted draft, signature-only check, or before the complete proof and
ordered execution could permanently burn a nonce for a rejected transfer.
An out-of-band write would not share rollback, persisted-but-unapplied replay,
or already-applied restart semantics with the asset and budget writes. No
production-correct isolated nonce invariant can be added while this carrier
and verified application source are absent; the existing refusal tests already
cover the current closed path.

The smallest sound implementation cut is an **atomic first-release carrier
replacement**, not an additive compatibility field: replace the envelope's
fragment collection with the exact signed `AxtAnchoredSpendV1` values; route
the syscall, CoreHost, block validation, persisted block, and replay through
one authoritative resolver; prove successful ordered execution, full state
roots, issuer authority, exact amount/proof and finalized anchor; then stage
`AxtAnchoredSpendReplayKeyV1` in the same `StateTransaction` as the successful
asset, budget, and envelope effects. Reject a key already in the parent WSV or
staged in the candidate. Persist it conservatively without expiry until a
sound bounded lifetime is justified. Only then can tests demonstrate
same-block duplicates, changed-intent reuse, failed-transaction rollback,
fork rollback, checkpoint recovery, persisted-but-unapplied execution, and
already-applied replay without double consumption. Those tests need the final
compact proof source; fixture signatures alone cannot open admission.

The current FASTPQ compact proof remains above its production bounds and Core
still uses witness replay. This record does not connect Core to the unqualified
compact verifier or claim F07 completion. No Rust source was edited and no
Cargo command was run for this audit.

Source anchors: [signed spend model](../../../crates/iroha_data_model/src/nexus/axt.rs),
[CoreHost AXT path](../../../crates/iroha_core/src/smartcontracts/ivm/host.rs),
[block admission](../../../crates/iroha_core/src/block.rs),
[State application](../../../crates/iroha_core/src/state.rs), and the
[previous State-owner analysis](../2026-09-23/axt-anchored-spend-nonce-state-owner.md).
