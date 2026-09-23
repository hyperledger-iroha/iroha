# AXT anchored-spend nonce State-owner boundary

This audit is against the `optimizations` checkout on 2026-09-23. The
`AxtAnchoredSpendReplayKeyV1` identity is defined, but there is no sound
standalone nonce-ledger insertion point yet. Its insertion must be part of
the first-release anchored-spend cutover, using the same successful
transaction State owner as the corresponding transfer, handle budget,
completed envelope, and asset writes. No nonce admission was enabled by
this record.

## Source trace

- `crates/iroha_data_model/src/nexus/axt.rs` defines
  `AxtAnchoredSpendV1::replay_key_v1` from the full issuer context and a fresh
  nonzero nonce. The type's own TODO says to replace the envelope's
  `AxtHandleFragment` collection and route block admission, CoreHost, and the
  syscall through one WSV resolver. `AxtEnvelopeRecord.handles` currently
  carries only `AxtHandleFragment`, so canonical block/application evidence
  has no anchored-spend nonce to consume.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` retains a completed
  `HostAxtState`, materializes an `AxtEnvelopeRecord`, and flushes it into
  `StateTransaction::record_axt_envelope`. The host rejects unanchored remote
  spends before completion. Neither the host state nor this materialization
  carries `AxtAnchoredSpendV1` or its replay key.
- `crates/iroha_core/src/state.rs` owns atomic candidate-side handle-budget,
  policy-counter, and handle-replay updates in `record_axt_envelope`. On
  admitted carrier replay it reconstructs these from the exact envelopes in
  `record_replayed_axt_envelope`; the World storage fields are serialized,
  transactionally reverted, projected into snapshots, and restored at
  recovery. This old handle replay key can be pruned after its retention
  window. It is not the once-only signed-spend nonce identity.
- `crates/iroha_core/src/block.rs` rejects any block envelope with remote
  spend handles after its current checks. The rejection covers block paths
  constructed outside CoreHost as well. The existing envelope therefore has
  no production-accepted spend from which to derive an authoritative nonce
  consumption event.

Adding a `Storage<AxtAnchoredSpendReplayKeyV1, ...>` field alone would be
inert. Consuming a nonce from a submitted draft or structurally signed model
before proof, finalized-source, and successful ordered execution checks would
burn it for a rejected spend. Consuming one from today's envelope would
require inventing a nonce absent from the canonical evidence. A sidecar or
post-commit callback would split it from the asset and budget transaction.

## Required atomic cutover

1. Replace the first-release envelope handle collection with exact signed
   `AxtAnchoredSpendV1` values and remove the retired fragment layout. Carry
   the same canonical bytes through the syscall, CoreHost, block validation,
   persisted block, and replay; do not add a parallel V1 route.
2. Resolve the authoritative finalized source anchor, issuer key and
   revocation, successful ordered transaction effects, full-state roots,
   exact proof statement, amount, expiry, and budget against the frozen State
   context. A structural DataModel signature check is insufficient.
3. Stage each `AxtAnchoredSpendReplayKeyV1` in the same `StateTransaction`
   that applies the successful transfer and its handle budget. Reject a key
   present in the parent WSV or already staged by another accepted spend in
   the candidate. A transaction failure must revert all effects, including
   the nonce; a successful transaction must persist all of them together.
4. Persist and replay the key through the canonical World field, snapshot,
   storage-transaction reverts, carrier projection, and cold recovery. Replay
   of already-applied execution must authenticate its exact consumed key
   rather than charge it twice. Do not prune a nonce while its signed spend
   may still be submitted; a permanent key is the simple conservative
   starting point, subject to measured storage admission.
5. Add adversarial tests for duplicate keys with changed intent/proof or
   handle, two same-block spends, failed transaction rollback, fork rollback,
   persisted-but-unapplied replay, already-applied restart, and a corrupt
   checkpoint missing a consumed key. Run them with the final compact proof
   and complete source relation, not just fixture signatures.

This keeps F07 open. The current hard refusal remains the correct production
behavior until the signed anchored-spend carrier and verified application
owner exist.
