# Core authenticated-history persistence

`KagemushaDiskAuthenticatedHistoryStoreV1` implements the existing history store
contract on Unix, including iOS and Android. It uses an owner-only directory and
one descriptor-locked, append-only Norito journal. The shared deterministic engine
validates a mutation first; the disk store writes and syncs its exact record,
rechecks file ownership, identity, generation and length, then applies the plan.
An uncertain write or changed file poisons the handle while retaining the last
acknowledged roots. Reads never substitute empty history for unavailable data.

A commit frame makes its immutable nodes, both selected roots, and terminal
transaction result durable together. Prepare/commit/abort retries preserve their
terminal outcomes without extra writes. The live overlay cap charges canonical
prepared-transaction bytes only. Committed nodes, records, and abort tombstones
have no count, age, or total-size cap. The fixed per-frame decode allocation bound
is independent of the live cap; lowering that cap cannot discard existing work.

The journal stores original hardware root-selection certificates, never serialized
verified capabilities. Both admission and replay verify signatures against a
Core-injected profile/epoch credential history. The current state must match its
exact hardware epoch and device-key reference. No signing keys, generated keys,
software monetary authority, or caller-controlled credential discovery are added.

Snapshots now include a deterministic recovery commitment over successful history
operations. It is part of the existing hardware-sealed snapshot commitment and
covers prepared work and abort tombstones even when both committed roots remain
unchanged. Recovery requires a retained matching checkpoint and matching roots.
Prepare/abort records after that checkpoint remain speculative evidence and are
retained exactly; a later hardware-authorized Commit cannot be treated as such a
suffix. A crash after durable Prepare but before a new anchor can therefore resume
the same prepared transaction without truncating the journal or inventing a commit.

`restore_from_disk_history` opens the concrete disk store and delegates to the
existing guard, snapshot, root, and proof-state validation. The supplied hardware
anchor must come from the current authenticated hardware session. A locally saved
old anchor is not a freshness authority. The product coordinator still owns exact
hardware operation reconciliation and publishing the corresponding private state
snapshot; a journal by itself cannot reconstruct proof witnesses or approve money.
New lanes use `create_new` and the existing fully verified `bootstrap` operation.
Missing/corrupt existing history never falls back to a new empty lane.

The tests exercise real P-256 signatures, canonical disk replay, exact identity
classification, current-key binding, process exit/reopen, writer exclusion,
corrupted/truncated records, valid same-root rollback, a speculative prepare suffix,
file replacement/tampering, and write/sync uncertainty. They qualify this persistence
component, not physical secure-element power-loss behavior or real recursive proofs.

Each prepared CAS now includes a required Core transition-attempt binding. Mint
and peer folds use the normalized hardware guard statement, which includes the
exact successor nonce and trusted time. Retrying that attempt preserves its
terminal result; a fresh authenticated attempt gets a different transaction ID
without deleting or reviving the old abort tombstone. Authorization and install
both check this binding. Commit retries must retain the original certificate.

The process-local index reuses validation only for immutable subtrees installed
by a durable Commit. New paths are validated completely, and every reused subtree
must satisfy the new parent edge's namespace, prefix, side and depth. Preparation
alone cannot populate that cache. Node replacement/removal clears the cached
validation; disk access also requires unchanged ownership and file generation.
The generic store default retains exhaustive validation. No cached summary is
serialized or accepted as proof or hardware authority.

Before a new hardware signing request or initial root-selection authorization,
Core now requires the exact transaction to remain prepared against the current
roots. This read-only preflight rechecks retained-tree/selected-path integrity and
preserves storage errors. An old preview retained after abandon cannot request
fresh authority. Idempotent abandon and exact already-committed certificate
recovery remain separate paths.
