# Final-promotion two-floor recovery design — 2026-09-24

**Design with a bounded private-journal implementation cut.** The exact signed
Current Check and Reserve can now be staged before Reserve transport. The
independent floor issuer, native pin, two-floor Core proof, production provider
and restart reconciliation below remain unimplemented. This is one proposed V1
path, not a compatibility path or a promotion qualification result. Software
custody is sufficient; no HSM is prerequisite.

## Existing seam and the trust gap

`FinalPromotionCheckExpectedV1.floor` in
[`observation.rs`](../../../crates/iroha_core/src/query/final_promotion_authority/observation.rs)
is simultaneously the current monotonic Check floor and the floor from which
[`reserved_source.rs`](../../../crates/iroha_core/src/query/final_promotion_authority/observation/reserved_source.rs)
replays to Reserve. `authenticate_applied_check_v1` in
[`signer_check.rs`](../../../crates/iroha_core/src/query/signer_check.rs)
opens one `StateView`, proves the signed Check and aligned successful result,
then walks State block hashes, durable Kura receipts, revision-4 signed RS16
finality artifacts and successor height contexts from that floor. The Reserve
consumer borrows this same view and proves the original signed Reserve against
its immutable admission row. After the floor advances beyond Reserve, it cannot
serve as the old source floor. Supplying the old tuple as the new Check floor
would roll back the Check trust boundary.

The daemon's `FinalPromotionRetainedFloorV1` currently exposes only `read` and
`advance_and_readback`; it has no historical pin, durable generation or
recovery factory. [`reserved_observation.rs`](../../../crates/irohad/src/signer_operation/final_promotion/reserved_observation.rs)
retains the original signed Reserve and pre-Reserve floor in its live owner and
now also pins an exact private pending record. Its pre-submit equality reread
prevents a changed floor from reaching transport in that process. The bounded
same-height preflight also matches the floor's height/hash/context to the local
State/Kura/QC artifact and durable receipt. The journal does not independently
issue an old pin, protect floor-store rollback after restart or prove a whole
successor interval. A private file, even with a digest chain, cannot by itself
prove that it existed before Reserve was submitted or that its highest
generation survived a rollback.

## Bounded journal cut in the current checkout

[`pending_reserve_journal.rs`](../../../crates/irohad/src/signer_operation/final_promotion/pending_reserve_journal.rs)
uses a dedicated owner-only `pending-reserve-v1` directory beside the receipt
journal. It retains a canonical intent and floor (intent at most 4,096 bytes),
the original signed Current Check and role-15 Reserve frames (each at most
65,536 bytes), and exact entry hashes in one immutable record (at most 136 KiB).
The pinned-file substrate retains its exclusive lease and durable fsync/readback
checks; receipt purpose types and byte ceilings are unchanged. Signed frames
are canonical and signature-verified. An operation-specific in-progress file is
synced as a one-use tombstone before its signed bytes are written. Only a
complete mode-0400 file is published under the recovery name with atomic
no-replace rename and a directory sync. An interrupted tombstone consumes the
journal budget and blocks reuse of its ID without blocking recovery of other
completed records. Live Reserve submission rechecks its pinned record first.
Reopening after a restart exposes read-only evidence and no signing or
resubmission method.

Scoped locked daemon tests pass: private journal 5/5, final-promotion Current
observation and Reserve handoff 18/18, and original receipt journal 9/9. After
the in-progress staging repair, current-source focused daemon selectors passed:
`scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p irohad
--lib signer_operation::journal::tests -- --nocapture` (13/13, including four
pending-Reserve crash/capacity tests) and the same command with
`signer_operation::final_promotion::pending_reserve_journal` as the selector
(5/5). These exercise exact bounds, forged signature, partial record, path
identity, restart readback and pre-transport tampering. They do **not**
authenticate a historical two-floor source after restart or reconcile an
ambiguous native Reserve. The native pin, independent floor generation, funded
source replay and production provider remain open release gates.

## Proposed canonical V1 contract

Introduce a purpose-specific `FinalPromotionReserveFloorPinV1` Norito record,
with network and deployment, operation ID, reviewed request digest, original
Current Check entry hash, **complete original signed Reserve entry hash**,
role-14/role-15 binding and key revisions, and a `FinalPromotionCheckFloorV1`
height/hash/context. It also carries a separate floor issuer identity,
generation, predecessor digest, and signature over the domain-separated
canonical record. The issuer must be governed, revocable authenticated
software custody independent of the Reserve signer. Canonical decoding rejects
missing fields and retired layouts.

The floor pin must become native evidence **before Reserve**: add one
purpose-specific `PinReserveSource` action and immutable admission index to
the final-promotion authority. Its authorized observer submits the signed pin
after the exact Reserve envelope is signed but before that envelope is sent.
The pin action records the signed Reserve entry hash without a circular
reference; Reserve does not commit the later pin entry hash. The Reserve
instruction itself must commit the original finalized Current Check entry hash
selected by the role-15 preparation. Native Reserve requires one matching,
already finalized pin for its own entry hash, request, authority and original
Current Check, and consumes that pin once. The original Current Check must
execute successfully at a lower height under the separate observer permission.
The issuer's certificate and native pin together establish independent
provenance and pre-Reserve ordering; neither a candidate row nor a decoded
signed envelope can issue a pin. The exact pin schema, permissions, and
retention index need DataModel/Core review before implementation.

Keep one `PendingFinalPromotionCheckV1::verify_finalized` entry point. Its
`FinalPromotionCheckSourceV1::Reserved` argument becomes a typed borrowed
`FinalPromotionReservedSourceEvidenceV1 { original_current_check,
original_pin, original_reserve, floor_pin }`; `Current` remains the only
source-free variant. There is no unbound Reserved overload. The Check expected
value keeps the **current monotonic** floor, while the source evidence supplies
the independently issued **historical** pin. Core verifies the issuer against
the separately configured/governed identity and revision, pin signature and
native pin admission, original Current Check, exact signed Reserve External
bytes, entry hash/index and successful ordered result, and the immutable
Reserved row. A pin for another request, operation, envelope, network,
deployment, issuer revision or fork fails before clock use. Completed phases
stay rejected until their separate signed Complete source exists.

Refactor the signer Check/Reserve lineage walker into a bounded internal
`authenticate_final_promotion_two_floor_cut_v1` called inside
`verify_finalized` **after** the signed Check has located its actual applied
State, retaining the same borrowed `StateView`. It verifies a single canonical
source-floor → native pin → Reserve → current-floor → fresh Check/applied-cut
sequence, matching both floor hashes and context IDs at their respective
heights, every intervening parent link, Kura receipt, signed RS16 artifact and
`VerifiedHeightContext::successor`. It then checks the three exact signed
entries' membership and aligned successful outputs against that cut. The
current floor must be a descendant of the source floor; it may be past
Reserve, and the source pin must predate Reserve. The existing 4,096-height
and 64 MiB finality-frame caps are upper bounds for the **whole** historical
walk, including any overlap; add a pre-I/O memory, archive-handle, work and
byte reservation under F02 instead of relying on post-hoc counters. A gap
beyond the cap fails closed and requires an independently audited compact
certificate design, never a silent skip or arbitrary older floor.

The returned `VerifiedFinalPromotionCheckV1` stays move-only and tied to its
new challenge, exact applied cut and original short monotonic lifetime. A
recovered signed Check is evidence for floor reconciliation only; it cannot
recreate a lost `NativeCheckRoundV1` or renew eligibility.

## Durable daemon transition

Replace the in-memory Reserve owner with a separate bounded, owner-only,
exclusive-leased journal patterned on [`SignerReceiptJournalV1`](../../../crates/irohad/src/signer_operation/journal.rs),
but with its own purpose and budget. Its canonical record retains the original
signed Current Check, Reserve and pin External frames (never private keys),
reviewed request and role bindings, floor certificate and generation, each
entry hash, journal sequence/predecessor digest, and the signed observer Check
frames used later. Enforce canonical size/count limits, no symlinks or
hardlinks, pinned directory descriptors, fsync of record and directory,
durable readback, and exclusive ownership. The configured floor service must
also provide an authenticated monotonic generation/readback and a durable pin
issuance operation; a caller-supplied old tuple is only input to be checked.

The ordered transitions are:

1. Persist a one-use role-15 key intent and the verified Current Check
   reference. The role-15 provider must durably return or reconcile the **same**
   signature; ambiguous key use with no recoverable result stops the operation.
2. Persist the exact signed Reserve frame and independently issued floor pin
   certificate before any native pin or Reserve transport. Persist the exact
   signed native pin frame before pin transport.
3. Reconcile pin submission against State/Kura/QC until its **successful**
   finalized entry precedes Reserve; then reread the floor generation and
   submit only the original signed Reserve frame. Persist each ambiguous
   submission state before a network retry.
4. Reconcile Reserve by exact entry hash and immutable origin. Obtain a fresh
   observer Check from the **current** authenticated floor, persist its signed
   frame before transport, and run the Core two-floor proof. A transport
   acknowledgement or decoded native row never advances authority.
5. Atomically compare/advance/read back the current floor and its generation.
   A crash between floor persistence and journal completion is resolved by
   reading the independent floor and exact finalized Check, not by rolling it
   back. Resample qualified UTC and use the move-only verified Check only in
   the live process. After restart, issue a **new** challenge and reprove the
   historical Reserve with the retained pin and then-current floor.

Every stage reserves physical bytes, nested allocations, archive reads, I/O,
work and deferred tombstones before key, file or network side effects. The
floor issuer and journal must agree on operation ID, generation, predecessor
digest and original signed Reserve hash. Replayed, revoked, foreign-key,
forked or lower-generation pins fail closed. A rejected or pending Reserve
cannot be promoted into a Reserved Check. No recovery path re-signs a Reserve
or treats a deserialized marker as `VerifiedFinalPromotionCheckV1`.

## Required tests and inputs still open

Use the real Core State/Kura/RS16-QC fixture. Prove a positive restart where
the current floor has advanced past Reserve, and negative cases for a
post-Reserve pin, wrong Current Check, same-block duplicate Reserve, signed
Reserve substitution, missing pin membership/output/finality, changed
committee context, forked or lower current floor, expired reservation,
revocation and a source-to-applied span exactly at and beyond each cap. Crash
the daemon at every numbered durable transition, especially after key use but
before signature persistence, pin finality, Reserve acceptance, Check finality,
floor CAS, and floor persistence before journal completion. Assert no duplicate
key use, no lost pending owner, no rollback and no final authority from an old
challenge. Include bounded allocation/I/O fault injection and forged journal
and issuer certificates. The existing fixture-only `RetainedFloor` is not a
production issuer or rollback store.

Production still needs a governed independent floor issuer, finalized native
pin producer, durable floor/journal deployment, role-15 recoverable key-result
protocol, qualified clock, approved fee/observer transport, and F02 funded
replay limits. Operator configuration must identify the issuer and its
revocation source before Core accepts a pin. Independent review must validate
the pin/committee trust argument and failure model. Until those inputs and
tests exist, production Reserved signing, Complete phases and promotion remain
closed. This note provides no release evidence.
