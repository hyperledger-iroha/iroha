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

The bridge aliases Core's sender creation context, including the native-authenticated
Core authorization key reference. Canonical input digests and guarded operation records
bind that reference exactly. Nonzero shape checks do not authenticate a key, and retained
context checks still establish scope only; the native session owns credential/key admission.

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
New lanes use `create_new` and the opaque `stage_bootstrap` owner. It exposes only
checkpoint material until both initial hardware CAS and fresh selection succeed.
Missing/corrupt existing history never falls back to a new empty lane.
Credential provisioning and concrete disk create/restore entry points currently
have Core test callers; product coordinator integration remains open.

The separate coordinator operation journal binds its initializer to the exact lane
and asset incarnation and reserves caller-persisted identities against exact public
bindings. Sender bindings use the tagged canonical Norito public-input enum; nested
send requests must pass canonical decoding, shape and wallet-scope validation before
an append reserves capacity. The C/JNI boundary decodes every other reservation using
its actual typed device-command codec. The transport-only frame validator does not
establish typed admission. Exact accepted retries survive reopening under a lower
capacity budget. The journal reconciles every Core-owned operation against the
actual outgoing index; retained public intent never becomes a Prepared capability.

A sender journal allowance is returned only after the exact Core Released record is
fsynced. Complete ID, binding, intent and terminal tombstones remain retained. Replay
also compares every terminal marker to the exact Core index: a journal-ahead release
cannot reopen against an older Core snapshot. A lost retirement response cannot
return capacity twice. The Core outgoing index and outbox meter likewise charge
only live allowances; historical record bytes remain separate storage telemetry.
Real storage exhaustion still fails before acknowledgment and cannot evict history.

Observation reads (device operations 1, 13, 18 and 21) cannot enter this durable
journal. Coordinator method 11, BeginObservation, accepts the operation and exact
canonical read body and returns one transient native-generated challenge. The owner
bounds outstanding observations and invalidates old challenges on replacement or
recreation. Accepted reads used to finish durable work must first enter that work's
authenticated dependent transcript; no old read can become fresh after restart.
Other durable command allowances require authenticated lifecycle completion.
The production coordinator must bind
the Core snapshot, operation-WAL head and response-journal head under one fresh
hardware checkpoint, authenticate historical release/credential bindings, and validate
any speculative suffix. Current operation WAL bytes alone are not rollback protection,
and an old authenticated observation must never be republished as a fresh read.

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


Core recovery snapshots require complete metadata: the exact original governed
credential and its admitting authenticated release, a global `u128` metadata revision,
coordinator and response journal prefix sequence/hash/length, and the hardware
response-history root and retirement transition identity. Monetary epoch rotation
never resets this metadata. A same-epoch credential cannot reduce issuance time;
at equal issuance time the entire original credential must match, including expiry
and issuer signature. Restoring a historical floor requires its separately
threshold-authenticated release capability, not a profile decoded from the snapshot.

`prepare_recovery_checkpoint` and `prepare_credential_checkpoint` derive immutable
proposals without publishing them. `stage_recovery_checkpoint` consumes the usable
machine into an exclusive pending owner. After the native owner durably persists
the exact candidate, `finish` requires the exact hardware CAS terminal certificate
and a verifier-owned fresh selection that also verifies the actual journal material.
Errors cannot return an older usable machine. Recovery then authenticates the
persisted candidate and hardware's current checkpoint. Immediate exact retries
retain original terminal certificate bytes; changed certificate bytes conflict.

The initial response-history root is the depth-256 SHA-256 SMT root, recursively
hashing empty branches above the domain-separated empty leaf. It is not the leaf
hash itself. Core treats proposed journal/root fields as structural claims until
the qualified native guard verifies actual persisted bytes and hardware selection.
The two new production guard hooks default to rejection. TODO: wire them to the
qualified platform transaction and exact native journal/archive descriptors; this
Core component alone does not enable a production coordinator or response deletion.
Checkpoint responses remain in the hardware terminal slot before appending them
to the response archive, avoiding a commitment that includes its own signature.


Production `stage_bootstrap` derives the immutable retail account/FI/dataspace/asset/lane
binding and original credential from an opaque verified enrollment certificate. Its
verification instant must equal the hardware-bound bootstrap time. The only synthetic
owner entry is compiled for Unix unit tests. Historical restore instead compares the
exact caller-selected owner with the hardware-selected complete snapshot; it does not
require current KYC or a renewed enrollment certificate to recover committed work.

The verified initial owner publishes a single journal bundle with atomic no-replace
rename. The bundle contains both initialized operation/response journals and an immutable
canonical complete initial snapshot journal. Thus explicit resume cannot change the owner,
credential, operation ID, capacities, or any snapshot field even before the first hardware
CAS. Partial private staging never occupies the final bundle path. Resume requires all
three complete original files and never recreates a missing child. Descriptors stay locked
through publication, and finish rechecks the retained snapshot file and both journal
prefixes before accepting the original hardware certificate. Surviving complete frames
are fsynced together with directory entries before replay can expose a recovery prefix.
All existing file opens are nonblocking before regular-file validation, so a substituted
FIFO cannot stall storage recovery or an ownership check.

Native startup can use the sealed `KagemushaCurrentRecoveryOwnerV1` projection
without naming or constructing a private history store. The returned borrowed selection
requires the complete current snapshot and anchor to equal the published checkpoint,
including uncheckpointed inbox or journal changes. The qualified guard must then
freshly authenticate the current hardware checkpoint and actual journal prefixes;
Core repeats full local equality after that exchange. The borrowed view exposes the
immutable owner, original credential floor, current hardware epoch/key and original
checkpoint only. Native open must separately verify its account/device possession
challenge; this metadata selection grants no new-work or monetary authority.
