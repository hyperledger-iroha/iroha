# Final-promotion operation-source audit — 2026-09-24

This source audit of the `optimizations` checkout records the implementation
boundary and the bounded causal-origin cut made on the same date. It is not a
production authority or promotion qualification result. No HSM is required.
There is one V1 protocol path.

Native role-14 operation storage is present. Core's
[`Reserve`/`Complete` executor](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_final_promotion_authority/operation.rs)
checks current custody, audit predecessor, exclusive reservation, fence and expiry.
The [shared mutation owner](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_final_promotion_authority.rs)
atomically stages immutable revision, height and admission indexes plus the mutable
operation slot and head. The [same-State reader](../../../crates/iroha_core/src/query/final_promotion_authority.rs)
reconstructs the custody control, audit head and selected operation at one height.
There is no missing native Reserve/Complete row schema to invent. A decoded row
alone is not finalized authority.

Core also has a purpose-bound finalized [role-14 Check consumer](../../../crates/iroha_core/src/query/final_promotion_authority/observation.rs).
Its [shared proof owner](../../../crates/iroha_core/src/query/signer_check.rs)
matches the exact applied signed Check entry, aligned successful execution result,
retained State, Kura commit receipts and revision-4 committee/QC successors from
an independently pinned floor, then rechecks the current same-cut custody,
operator/observer permissions and requested operation phase. A `Current` Check
authenticates the custody and audit pair. Native execution accepts
`BeforeProvider`, `AfterProvider`, `BeforeCommit`, `AfterCommit` and
`BeforeRelease` Checks against their respective rows, but the consuming proof
must also establish the original signed operation source. Every phase needs a
fresh Check.

The daemon's [Current Check runtime](../../../crates/irohad/src/signer_operation/final_promotion/current_observation.rs)
consumes the Core result and returns a bounded custody/audit observation, but
its independent observer, approved transaction construction, exact submission
and reconciliation, qualified UTC and rollback-protected floor have only
injected interfaces and test implementations. It has no Reserve or Complete
method. Its consuming `into_reserve_context()` now transfers the original
move-only verified Check together with the same-cut signing state for
[role-15 account preparation](../../../crates/irohad/src/signer_operation/final_promotion/account_transaction.rs),
which requires that exact Check to derive Reserve. This daemon handoff is
covered by the focused Current/Reserved suite recorded below. The
[role-15 software key](../../../crates/irohad/src/signer_operation/final_promotion/account_transaction/software_key.rs)
signs only the typed, already-authorized account request. It does not submit or
verify finality. The generic `SignerOperationStateSourceV1` still has no production
implementation; the role-14 four-signature service therefore has no finalized
Reserve/Complete source. Its coordinator observes the source in its constructor,
so a production factory must pin the independently reviewed request and original
custody identity before constructing that coordinator.

An exact Reserve applied-entry proof had a **V1 causal-origin
prerequisite**. `FinalPromotionExecutionV1.ordinal` is an ordinal in the
deployment's operation history within a block, not the network entry index.
Before this cut, `FinalPromotionOperationRecordV1` retained a request digest
but no originating network entry hash/index. Two differently enveloped,
same-action Reserve transactions signed by the same operator could both execute
successfully in one block: the first allocated the row and the second took the
executor's idempotent retry branch. Their successful entry proofs and the same
matching row did not identify which envelope allocated it. A same-height
request-digest comparison was insufficient to claim exact signed-envelope-to-row
causality.

The bounded V1 cut now records the originating network entry hash and index
in the immutable operation row and requires a direct, single, signed role-15
Reserve/Complete instruction when the row is created. Core already exposes
`StateTransaction.current_network_entrypoint_hash`,
`current_entrypoint_index`, `tx_call_hash` and `current_tx_hash`; the
[role-11 direct-source check](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_stream_token_authority.rs)
shows the closed-purpose pattern for rejecting sealed outer/inner confusion.
The [executor's direct signed-instruction check](../../../crates/iroha_core/src/executor.rs)
is the relevant entrypoint gate. These fields must be bound at mutation time,
then compared with the exact authenticated entrypoint proof and aligned
successful result at readback. The row's current `execution_origin` names the
Reserve or Complete action that made that revision; `reserved_origin` retains
the original Reserve entry through every terminal outcome. Expire and custody
invalidation have no direct role-15 `execution_origin`. Idempotent Reserve and
Complete success now requires the identical stored origin; a distinct signed
envelope fails even when its intent and authority match. Native readers reject
missing or inconsistent origins, and the canonical decoder does not accept the
pre-origin layout. This changes one canonical first-release layout; there is no
compatibility decoder or alternate operation path.

The executor's signed-entry validation currently clones the bounded native
envelope and constructs its canonical frame. The 64 KiB envelope limit does
not reserve that allocation or the later nested work. F02 resource admission
must fund it before production resource qualification; this remains open. The
role-15 custody check observes the authenticated committed parent State at the
current logical block time, without extending the signed enrollment's expiry
or changing its active-head anchor. An independent finalized Current Check is
still required for the daemon's operator observation.

The bounded Core cut gives `PendingFinalPromotionCheckV1::verify_finalized`
one mandatory, phase-matched source argument. `Current` has no prior operation;
the three Reserved phases require the original signed role-15 Reserve envelope.
The consumer reads the immutable admission-indexed Reserved row, requires the
independently retained Check floor to predate Reserve, and authenticates the
original entry hash/index, complete signed External bytes, aligned successful
result, request, role-15 account, network and block time against the same
borrowed `StateView` used for the challenged Check, current custody and
permissions. It replays bounded signed-RS16 State/Kura/QC lineage from that
floor to Reserve; the Check proof authenticates the same lineage onward to its
applied cut. The 4,096-height/64 MiB Reserve replay caps do not fix the shared
Check path's outstanding F02 resource admission. A decoded row, a later mutable
operation slot or a self-selected post-Reserve floor cannot grant authority.
The production daemon Current caller uses the same API with the `Current`
variant. Its future Reserved caller must retain the original signed envelope
through submission and ambiguous-result reconciliation, then obtain a fresh
`BeforeProvider` Check before role-14 key use. The rebuilt Core binary passed
`reserved_check_requires_original_source_membership_and_durable_reserve_finality`
1/1, and the full final-promotion observation module passed 41/41 on
2026-09-24 (`target/f04-observation-post-adversarial.log`). The adversarial
case executes a genuine signed Reserve, then separately removes original
source membership or signed-RS16 finality and requires the consuming Check to
reject before clock use. No production finalized operation source has been
connected.

A bounded daemon `BeforeProvider` owner now consumes the move-only role-15
signed Reserve owner, pins a floor
before submitting that exact envelope, reconciles only the original bytes, and
uses one real State snapshot to prepare the fresh observer Check. It delegates
Reserve origin, successful execution and finality proof to Core, then advances
the independent floor. The post-persistence clock step retains the verified
Check in memory on a failed sample so a fresh sample may be attempted within
the original lifetime. This is **not restart recovery**: a crash after the
floor advances loses the in-memory Check, and that advanced floor cannot replay
the original pre-Reserve source. A durable pending-envelope/pre-Reserve-floor
recovery journal and qualified providers remain production blockers. The
combined daemon Current/Reserved observation suite passed 16/16 on 2026-09-24
(`target/f04-daemon-floor-preflight.log`), including original signed
Reserve reconciliation, substituted/unapplied Reserve rejection, zero transport
on a concurrently advanced pre-submit floor, rejection of a wrong same-height
finalized committee context, and in-memory retry after a post-persistence clock
failure. These fixture-backed tests do not qualify a
production floor issuer, restart recovery or signing deployment.

The owner rereads the independent pre-Reserve floor immediately before Reserve
transport and rejects a changed value without submitting. Construction and
pre-submit use one real State view each to match the retained floor's exact
height/hash/context against `verify_signer_finality_v1` and the durable Kura
finality artifact/receipt; a false context now fails before transport. This
authenticates one finalized coordinate against the local State/Kura/QC cut, not
the floor issuer's independence, rollback protection or full successor lineage.
Core's context-pinned successor walk is private to the post-application Check
proof. A production floor factory and F02 pre-I/O memory, archive and work
reservations remain open; the existing post-hoc bounds do not fund this read.

**Durable Reserve recovery design (not implemented).** The source-owned V1
journal must retain an immutable canonical signed External Reserve frame (at
most the existing 64 KiB native envelope), its entry hash and payload digest,
the reviewed request and complete role-14/role-15 binding identities, and the
independently read pre-Reserve floor's height, block hash and height-context
identity. It must also retain each exact signed observer Check frame/challenge
before submission, the phase, the then-current Check floor, the resulting
floor-CAS generation and applied coordinates, and an append-only state/digest
chain. Stages need a pre-key one-use intent fence, signed-Reserve persistence
before transport, submission/ambiguity, exact Reserve application, signed
BeforeProvider Check, exact Check commit/finality, floor CAS/readback,
post-persistence clock qualification and final consumption.
No private key belongs in this journal. The existing immutable private receipt
journal is a different owner and cannot stand in for these operation transitions.

On restart, an exclusive owner must authenticate the bounded canonical journal
and software authorization, compare its binding/revision and reviewed request
with configuration, read the current independent floor, and reconcile only the
original signed entry hashes against State, Kura and QC. A pre-key fence with
no durably recorded signed result is ambiguous and must not call the role-15
key again. An accepted or submitted envelope without exact finalized successful
execution grants no phase authority. A lost local monotonic Check lifetime also
cannot be revived by deserializing a prior verified marker; recovery must issue
a fresh observer challenge after proving the original Reserve and current
custody. A floor CAS and its journal transition need one recoverable atomic
protocol, including the crash window after floor persistence and before the
journal records completion. Every transition must have an explicit byte/count,
memory, I/O and retained-tombstone reservation from configuration before
allocation or key/network I/O, plus owner-only no-symlink storage and durable
readback.

The current Core verifier cannot yet perform that restart proof: one
`FinalPromotionCheckExpectedV1.floor` serves both the monotonic Check proof and
the historical floor-to-Reserve replay. After the global floor advances beyond
Reserve, that floor fails the source pre-Reserve condition, while reusing the
old source floor as a new Check floor would roll back trust. A revised Core
consumer needs two explicitly different trust inputs: the current monotonic
floor for the fresh Check and an immutable pre-Reserve source pin retained
with the operation before its submission. It must authenticate both pins and
their ancestry in the same canonical State/Kura/QC cut. The historical pin
must be issued and retained by the independent floor owner, with durable
operation binding and authorization; Core must not promote a caller-supplied
old coordinate, decoded row or recovered candidate into a trusted pin. An
equally strong durable Reserve-source certificate could replace the historical
replay, but neither API exists now. Even with such a change, a new
Check must respect the 4,096-height/64 MiB replay cap, reservation expiry,
revocation, qualified UTC and bounded spending. The role-14 operation journal
must separately fence protected key use and completion. None of these recovery
prerequisites is implemented by the in-memory post-persistence owner.

`AfterCommit` and `BeforeRelease` verification rejects because the canonical
API has no Complete source variant. Complete needs the staged four-signature
receipt, an exact signed Complete transaction, independently authenticated
timely native completion, and fresh
`AfterCommit` and `BeforeRelease` Checks. Production assembly also needs the
observer signing service, bounded spending approval, submission/reconciliation,
qualified UTC, rollback-protected floor, role-15 Current Check driver, role-14
software provider and restart journal. Those interfaces and deployments remain
open, as do the independent promotion approvals. The prior bounded causal-origin
cut passed the coordinated Core low-level suite, 47/47 tests on 2026-09-24
(`target/f04-core-lowlevel-rebuild.log`). The earlier same-binary Core
observation module passed 40/40 before the new adversarial case was added.
These focused tests do not establish funded resource replay, production signer
integration, or promotion readiness.
