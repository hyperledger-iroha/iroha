# Merge ledger: certified lane execution and global ordering

This document defines the live merge-ledger validation, replay, and recovery
contract used when `nexus.enabled = true`. A lane certificate proves that a
lane payload is available and accepted by its lane committee; it does **not**
authorize a shared-world-state mutation by itself. Only a canonical global
block carrying a merge-committee certificate establishes the order in which
lane effects enter WSV.

The production runtime constructs and hands off bounded autonomous payloads,
collects their availability and lane-finality evidence, builds certified merge
candidates, serves missing authenticated execution sidecars, and collects lane
drain votes. One exact durable reservation identity follows each transaction
from FIFO queue acquisition through the lane payload, merge certificate,
canonical global application, and final queue retirement.

Legacy single-lane operation continues to use ordinary global blocks. The
direct lane-application helpers retained in tests model historical and failure
cases; production builds recover certified lane inputs but never apply them in
QC-arrival order.

## Safety and liveness invariants

The implementation enforces the following validation and replay invariants for
every accepted encoded form:

1. A non-empty autonomous lane block has a producer-authenticated payload, an
   availability-backed prepare QC, and a commit QC from the authoritative lane
   committee.
2. The deterministic leader for one exact global `(epoch, view, height,
   parent, validator-set)` round proposes one canonical merge body.
3. Every honest merge validator re-executes that exact body from committed
   state and embedded evidence. Local sidecar arrival order and opportunistic
   inventory never choose the body a follower signs.
4. A validator durably records its complete signing context and candidate
   digest before exposing a signature. Restart cannot erase or change that
   decision.
5. A merge QC authorizes exactly one global carrier height, parent hash, view,
   chain, ordered roster, candidate, lane binding set, and execution result.
6. The full entry is hash addressed. The globally ordered block contains a
   bounded compact reference, not the multi-megabyte body.
7. WSV stages the resolved entry on a pristine block overlay before lifecycle,
   start-of-block, trigger, or ordinary transaction effects.
8. A missing live sidecar defers the block and starts authenticated fetch.
   Relay and autonomous execution entries use the same bounded,
   height-context-authenticated holder protocol.
9. Kura makes the block, full entry, and sparse carrier record durable before
   state publication. Recovery either reconstructs the same prefix or fails
   closed at the first corrupt/conflicting record.
10. Automatic retirement is a two-certificate-boundary protocol: a committed
    intent first closes the exact lane incarnation to new work; its historical
    lane committee then certifies the final globally applied frontier; a merge
    QC globally orders that certificate; and only a strictly later global block
    may retire the lane. Unmerged relay progress, unapplied certified execution,
    an incomplete Native finality/manifest/receipt/latest-pointer join,
    unrepaired application evidence, or frontier drift blocks completion.

## Candidate forms

`MergeLedgerCandidate` has three mutually exclusive forms.

### Autonomous execution batch

The deterministic lane author and global merge leader synthesize this form in
the live V2 runtime.

An execution batch contains one next-contiguous certified source from each
selected lane, sorted canonically by proposal height, lane-local height, lane,
dataspace, and view. Each `MergeLaneExecution` embeds:

- the exact framed producer-authenticated source bundle and its hash;
- the origin proposal and the current quorum-authorized proposal;
- availability-backed prepare and commit QCs plus signer proofs of possession;
- chain, epoch, payload, lane, dataspace, incarnation, and activation bindings;
- exact entrypoints, queue reservation keys, routing plans, and Native AMX
  receipts;
- deterministic results and result hashes; and
- the settlement commitment derived by the same execution.

The global leader executes the ordered sources on a revertible `StateBlock`
based on the current committed WSV. The batch commits the base height/hash,
stripped application header, entrypoint/result Merkle roots, execution root,
application write-set root, final write-set root, expected post-state hash, and
batch hash. Followers reconstruct the sources from this embedded material and
must obtain byte-identical executions, results, settlements, and roots before
signing.

### Relay-settlement snapshot

A relay candidate contains one next-contiguous `MergeLaneSnapshot` per selected
active lane. Every snapshot carries its exact `LaneRelayEnvelope`, settlement
commitment/hash, lane tip, merge hint, lane/dataspace/incarnation, and first
eligible height. Admission verifies the authoritative lane committee QC, DA and
FastPQ material, settlement coordinates/totals, and the merge-hint reduction.

Honest merge validators also preflight all deterministic WSV settlement
conditions before signing, including activation, duplicate receipt markers,
canonical fee assets, aggregate arithmetic, and payer balances. A candidate
that could not be staged on its exact parent therefore cannot obtain an honest
signature.

### Lane-drain certificate carrier

A drain candidate is certificate-only: it contains neither an execution batch
nor relay snapshots. The certificate body repeats the exact committed close
intent and final `(lane height, descriptor hash)` frontier. The intent binds
the chain, lane, dataspace, incarnation, close height, initial merged frontier,
and the exact ordered historical lane committee with its canonical quorum.
Each selected signer supplies a BLS proof of possession, and the aggregate
signature covers the complete body.

For an autoscale-managed lane, that committee and its aligned BLS proofs of
possession were committed before the lane incarnation hash was derived. Every
lane QC, availability certificate, NewView certificate, and Native AMX
participant check for the incarnation uses the same pin. Later global roster,
manifest, or key-cache rotation cannot rewrite historical authority and there
is no live-authority fallback. A pin change is a retire/recreate boundary.

Lane validators durably lock their full commit-vote bodies and highest signed
drain frontier per incarnation before either artifact can leave the process. A
drain frontier below a locally signed commit high-water is rejected; after a
drain decision, every later commit vote is rejected across restart. The same
intent may be re-signed only at a strictly higher canonical frontier when
delayed pre-close work is globally applied. Quorum intersection prevents an
earlier valid close certificate from coexisting with the lane QC needed to
advance that frontier.

Drain votes are bounded, authenticated control-plane traffic. They are sent to
both the historical lane committee and current global validators, because the
next deterministic merge leader need not belong to a bounded lane committee.
The first valid certificate assembled for an intent/body is frozen locally;
same-intent signer decisions survive frontier refreshes, accepting only
strictly monotonic advances and excluding same-height conflicts or regressions.

An entry never mixes autonomous execution, relay snapshots, and drain
certificates. This keeps all three replay protocols unambiguous.

## Active lane and lifecycle binding

Every candidate contains the complete canonical `active_lanes` vector. Each
`MergeLaneBinding` commits:

- `lane_id` and `dataspace_id`;
- the canonical lane configuration hash;
- a never-reused incarnation hash; and
- the first global proposal height eligible to use that incarnation.

`lane_catalog_hash`, `incarnation_root`, and `activation_root` bind the vector.
The internal reserved drain-state metadata is excluded from lane
configuration/catalog commitments because intent and commitment updates do not
reconfigure or reactivate the lane; the certificate and global carrier bind
that state explicitly. Every other lane-configuration change still requires a
fresh incarnation and later activation. Validation compares bindings with
committed Nexus state and rejects omitted, reordered, duplicated, stale,
future, or reconfigured lanes. Execution and snapshot histories are contiguous
within `(lane, dataspace, incarnation)`; retire/recreate starts a fresh
namespace and old artifacts cannot cross the activation boundary.

## Exact global round and merge QC

The global commit topology determines the round leader and the ordered merge
validator set. The signed payload is the canonical Norito encoding of:

```text
chain_id_digest
validator_set_hash_version
validator_set_hash
view
epoch_id
carrier_height
carrier_parent_hash
lane_catalog_hash
active_lanes
incarnation_root
activation_root
lane_snapshots
execution_batch
lane_drain_certificates
global_state_root
```

It is domain separated by `iroha:merge:qc:v1\0` and the configured chain ID.
The resulting digest is stored in `MergeQuorumCertificate.message_digest`.
The QC also embeds the exact historical roster, canonical LSB-first signer
bitmap, ordered signer PoPs, and aggregate BLS-normal signature. Validation
rejects duplicate validators, wrong roster hashes or versions, bitmap length or
padding drift, out-of-range/duplicate signer indices, missing/misordered PoPs,
quorum shortfall, invalid aggregate signatures, and any mismatch in the exact
carrier height, parent, or view.

### Leader body transfer and anti-equivocation

Relay, autonomous execution, and certificate-only drain candidates use the
same live leader-transfer and durable anti-equivocation boundary.

The leader first persists authorization in `MergeSigningGuard`, then announces
the canonical candidate hash, byte length, message digest, and exact round.
Candidate request/chunk messages are authenticated by P2P sender identity and
bound to the leader and announcement. Bodies use canonical Norito encoding and
are split into at most 64-KiB chunks.

Followers accept announcements only from the current deterministic leader,
fetch and assemble the exact body under global/per-peer byte and session caps,
verify its canonical bytes and announcement digest, and re-execute it. A
conflicting leader body causes view change. Signature shares received before
the exact candidate is installed are not sufficient to form a QC.

The signing guard is crash safe, refuses a second digest for the same complete
context, records a committed height/epoch high-water mark, and fails closed on
malformed, oversized, non-regular, or symlinked records. Ordinary global blocks
advance the height high-water mark too, so an idle merge ledger cannot exhaust
the bounded guard journal.

Lane-drain signing adds an exclusive per-directory `owner.lock`, so two node
processes cannot concurrently use the same durable voting identity. Drain-vote
transport applies the canonical 16-KiB frame and 128-validator sequence limits
before Norito materialization on both stream and QUIC datagram ingress.

## Compact global carrier and sidecar protocol

The complete `MergeLedgerEntry` includes the candidate fields and merge QC. Its
domain-separated canonical hash and exact encoded length form the primary
sidecar identity. A global block stores `CertifiedMergeLedgerReference`, which
contains:

- schema version, entry hash, encoded length, and merge epoch;
- optional execution batch hash, entrypoint count and entrypoint/result roots;
- optional base WSV height/hash; and
- the complete merge QC.

The compact reference must equal `CertifiedMergeLedgerReference::new(entry)`.
Static block validation and `StateBlock::stage_certified_merge_entry` both bind
the QC height, parent, and view to the actual carrier header, including
snapshot-only entries.

If a live sidecar is absent, validation returns a deferred missing-sidecar
outcome. The node derives eligible holders from the already-validated merge-QC
signer bitmap and rotates requests among them. Responders serve relay and
autonomous execution entries only when the request names the exact authenticated
height context and they are an eligible holder. Requester, responder, request
ID, reference digest, entry hash, length, chunk index/count, and payload hash
are checked before assembly. Incomplete assemblies are memory-only and
disappear on restart; only a complete canonical entry matching the compact
reference is persisted. Limits currently include 16 MiB per full entry,
64-KiB chunks, 32 global and four per-peer inbound sessions, 64 MiB global and
32 MiB per-peer assembly bytes, and separately bounded outbound/candidate
sessions.

Timeout uses capped exponential backoff. There is no fixed attempt or wall-clock
horizon for an authoritative pending block: withholding holders cannot convert
temporary unavailability into permanent rejection. Pending state is pruned only
when the canonical block is committed or superseded.

## Native participant application evidence

Native participant finality remains control-only; the globally ordered carrier
is the sole economic executor. For every participant height requiring separate
application, Kura stores one immutable versioned manifest file and one
immutable versioned receipt file. Both use create-new temporaries, no-clobber
promotion, directory durability sync, and exact readback. A byte-identical
same-height replay is idempotent; a conflicting identity cannot overwrite the
stable file. A separate route/incarnation-bound latest pointer is replaceable
derived state, published through its descriptor-bound directory and rebuilt
explicitly from the standalone files at startup.

The standalone histories are bounded by the configured Kura sidecar-retention
count and the existing shared Native sidecar aggregate-byte budget, including
one bounded transient publication slot. Pair compaction first fsyncs a
versioned intent naming the lane, dataspace, incarnation, and every
`(artifact kind, participant height, artifact hash)` to remove. Startup
recovers a publication temporary beside no stable file or an identical stable
file, a temporary-only or stable prune intent, every individual
manifest/receipt unlink stage, a completely unlinked pair, and identical
stable-plus-temporary prune intents. Completion is idempotent and must preserve
the exact pair named by the latest pointer.

Legacy dense Native data/index files and malformed, conflicting, unexpected,
oversized, non-regular, hardlinked, or symlinked artifacts fail closed before
interpretation or deletion. Drain, archive validation, retirement, purge, and
same-ID recreation use the same exact finality/manifest/receipt/latest-pointer
join, so retained evidence from an earlier incarnation cannot authorize the
new lane.

## Atomic commit and recovery

For a carrier block, commit proceeds in this order:

1. Resolve and validate the exact full entry. Stage relay-settlement effects or
   deterministically re-execute the autonomous execution batch on a pristine
   block overlay against the exact committed base WSV.
2. Commit the global block certificate in memory.
3. `Kura::store_block_with_merge_entry` durably writes the full entry, exact
   sparse `(entry hash -> carrier height/hash)` record, and canonical block as
   one rollback-safe operation.
4. Persist the V2 finality artifact and the execution commitment's canonical
   Native AMX application manifest. For each participant height, publish its
   immutable manifest file, immutable receipt file, and descriptor-bound exact
   latest pointer before its WSV frontier becomes visible.
5. Persist the staged WSV checkpoint, apply ordinary block effects after the
   already-staged merge effects, and atomically commit the WSV overlay.
6. Persist the commit manifest and exact merge-application receipt, repair any
   interrupted Native manifest/receipt/latest-pointer or pair-prune
   publication, then publish the entry into the bounded in-memory merge cache
   and emit one `MergeLedgerEvent` for the first live publication.
7. Only after canonical Kura and WSV application is durable, transition each
   exact reservation through Commit and ForgetCommit. Startup re-enters this
   boundary idempotently after any crash.

If Kura persistence fails, State is not published. If the process stops after
Kura succeeds but before State publication, startup only hydrates entries whose
carrier height/hash is present in the canonical committed block prefix. Future
log/carrier suffixes are rolled back before block replay.

The merge log is a framed append-only file with validated hash/epoch-to-frame
offsets and a maintained latest execution-height index. Hash lookup performs a
bounded seek and canonical re-decode instead of scanning history. Append,
truncate, prune, cache eviction, and reopen update the indexes exactly. Carrier
lookups use height and entry maps. Recovery rejects duplicate hashes/epochs,
non-contiguous epochs, oversized frames, checksum/canonical drift, conflicting
carrier identities, symlinks/non-regular files, and malformed complete temp
files; a torn tail is truncated to the last complete validated prefix.

Live publication is idempotent. Replay publication never emits a pipeline
event, and an entry already published or recovered cannot emit again.

Canonical chain pruning is also a forward-recoverable transaction. Before any
destructive truncation, Kura fsyncs a versioned intent binding the source and
target tips plus the retained merge prefix. The durable block marker is lowered
before block/index/data and DA suffixes are removed; carrier, merge-log, roster,
WSV-checkpoint, commit-manifest, pipeline-recovery, and roster-metadata sidecar
suffixes then follow. Indexed sidecar prefixes are compacted through synced
temporary pairs and recover across either rename boundary without discarding
the sole valid index. In-memory height and query indexes publish only after
every durable stage succeeds. A crash at any later boundary leaves the intent
for startup to finish idempotently, while malformed, conflicting, symlinked, or
non-canonical recovery material fails closed. Once the intent is durable, Kura
poisons canonical reads and serializes every lane-artifact writer behind both
the prune and lane-geometry locks; a writer cannot resolve a lane path while
scale-in moves that incarnation into its authenticated archive. Merge-height
lookup returns an error rather than an empty map, so a recovering store can
never be mistaken for a fresh lane incarnation.

## Query and proof semantics

Committed transaction queries combine ordinary block transactions with
merge-carried transactions reconstructed from Kura's exact carrier index and
full entry. Results remain newest-first and use the same predicate, pagination,
and fetch-size machinery.

Indexed predicates (block/entrypoint/transaction hash, authority, timestamp,
and result status) resolve only their selected carrier heights; corruption in
unselected history cannot amplify or poison that query. Unindexed paginated
queries use a fallible per-carrier visitor: at most one protocol-bounded full
entry, its two Merkle trees, and the current response page are resident at once.
Bounded pages may stop before older carriers and validate them when the cursor
reaches them; exact-count or sorted requests scan every selected carrier and
fail before returning on any inconsistency. Unsorted stored continuations retain
only a canonical height/tip anchor plus a `(height, intra-carrier offset)`
checkpoint, so every later page resumes from the last emitted transaction and
never rescans newer carriers. Sorted transaction windows are validated and
materialized once, capped at 4,096 positions, and then paged from that bounded
window; larger or unbounded windows fail with the query
materialization-budget error instead of allocating complete merge history.
Because the current visitor eagerly reconstructs a complete touched carrier,
its full ordinary-plus-merge transaction count is charged before Merkle trees,
proofs, or predicates are materialized. An insufficient gas budget therefore
fails before carrier-sized proof allocation even when an adversarial predicate
would reject every transaction.

For a merge-carried transaction, `CommittedTransaction.merge_inclusion`
contains the merge entry hash/epoch, batch hash, exact leaf count, and typed
entrypoint/result Merkle roots. The ordinary `block_hash` is the compact carrier
block. `verify_certified_merge_inclusion` checks aligned indices, exact proof
depth/count, entrypoint and result hashes, both Merkle proofs, and every compact
reference commitment. `verify_certified_merge_inclusion_in_block` additionally
binds the proof to the exact signed block hash and the carrier height, parent,
and view. Legacy encoded query results decode with `merge_inclusion = None`.

## Protocol limits

The shared data-model constants are authoritative:

- full merge entry: 16 MiB;
- execution batch: 12 MiB;
- ordered entrypoints per batch: 4,096;
- one autonomous source bundle: 4 MiB;
- certified-source reservation inside that bundle: 1 MiB;
- lane committee: 128 validators; and
- lane-drain vote frame: 16 KiB, decoded with bounded depth, elements, and
  allocation before actor-queue admission.

Lane proposal admission uses the same entrypoint/byte corridor so a lane cannot
certify work that is intrinsically unmergeable. Count and length checks precede
large allocation, PoP verification, aggregate verification, or WSV execution.

## Primary implementation and verification

The autonomous producer, lane drain collector, certified-bundle carrier, and
global re-execution path described above are production code. Autonomous
execution is not feature- or environment-gated: configured lanes reserve FIFO
work, certify it, merge it through the global carrier, and release or forget the
exact reservation according to the durable outcome.

- Data model: `crates/iroha_data_model/src/merge.rs` and
  `crates/iroha_data_model/src/block/execution_context.rs`.
- Candidate construction/re-execution and WSV staging:
  `crates/iroha_core/src/state.rs`.
- QC digest/reduction helpers: `crates/iroha_core/src/merge.rs`.
- Certified full-entry sidecar transport plus durable merge signing guard:
  `crates/iroha_core/src/merge_sidecar.rs` and
  `crates/iroha_core/src/sumeragi/v2_lane_work.rs`.
- Durable log/carrier indexes and crash recovery: `crates/iroha_core/src/kura.rs`.
- Global proposal/commit/apply wiring:
  `crates/iroha_core/src/sumeragi/v2_candidate.rs` and
  `crates/iroha_core/src/sumeragi/v2_apply.rs`.
- Complete committed-transaction query:
  `crates/iroha_core/src/smartcontracts/isi/tx.rs`.

The production-bound formal models
`SumeragiV2AutonomousReservationCarrier.tla`,
`SumeragiV2NativeApplicationEvidence.tla`, and
`SumeragiV2AutoscaleLifecycle.tla` cover the reservation/carrier, Native
application-evidence, and lane-lifecycle invariants. Focused unit and adversarial
fixtures exercise malformed QCs, candidate equivocation, corrupt or
out-of-order sidecars, bounded-resource abuse, crash points, restart replay, and
stale-incarnation rejection. Native evidence cases additionally cover
configured retained-count and aggregate-byte overflow, malformed/conflicting/
oversized publication temporaries, obsolete dense layouts, and every
temporary/stable/partial-unlink prune crash stage.

Those implementation and focused-test statements are not release evidence. All
multilane gates remain open until the focused/adversarial, source-bound formal,
cross-SDK, fresh unskipped four-peer, 10/10 twelve-peer corridor, two-hour fault
soak, pinned-hardware one-versus-four-lane scaling, and prescribed
full-workspace build/test/strict-Clippy checks have passed and their artifacts
have been archived.
