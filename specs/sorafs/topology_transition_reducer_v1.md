# Topology transition reducer V1

Status: prerequisite implemented as a pure reducer; native integration and qualification remain
open. This module supplies no signing/execution/finality capability. It depends on the distinct
role 16 topology receipt contract, whose generic signer and promotion gates remain closed.

`iroha_data_model::sorafs::topology_authority::reducer::TopologyStateViewV1` is the sole transition
planner. It borrows one retained summary, current custody and exact keyed reads; it does not own,
clone, scan or reconstruct the operation/key inventories. `TopologyTransitionModelV1` is an owning
cold-replay accumulator that calls this same planner, never a second live-State cache. Core must
reuse these rules rather than copy them into a second executor. Inputs are explicitly claimed coordinates until Core derives and verifies them from its
actual transaction, finalized state and scoped permissions. The output is an exact proposed delta,
not an authenticated operation or a durable-commit receipt.

The native caller must pin deployment, genesis network, chain and address discriminator from its
own authenticated State. `TopologyRetainedStateV1` commits the separate custody/operation/history
heads, audit, never-reset fence, active ID, permanent ID/key cardinalities and last mutation context.
Its bounded relationships and exact current custody record/decoded state are checked before keyed
reads. Every current key tombstone and active slot must exist and match; an indexed row for another
ID or deployment is rejected. Public construction of these claimed inputs establishes no authority. Configure delegates key/policy
transitions to Manifest's canonical custody reducer. Enroll verifies the independent attestation
against the exact predecessor, next sequence and claimed committed anchor. Revocation is monotonic
and needs no old signer signature. Every control change atomically invalidates an active operation.

Reserve retains the full reviewed candidate subject, canonical role 16 request, original Sign intent,
custody, owner and audit predecessor. It requires committed eligible custody, one empty active slot,
a never-used operation ID and capacity for both reservation and terminal record. The deterministic
reservation binds all original inputs and actual-to-be-supplied execution coordinates; its lifetime
is at most 60 seconds and is capped by custody, policy and subject expiry. Complete requires that exact
owner, intent, request and reservation in a later block before exclusive expiry, advancing audit
exactly once. Expire/Invalidate leave permanent tombstones and do not advance audit. Exact repeated
Reserve/Complete/Expire inputs can observe the original result without another mutation; they
cannot mint a new fence, renew custody or authorize signature release.

Current/BeforeProvider/AfterProvider/BeforeCommit/AfterCommit/BeforeRelease inputs retain a nonzero
consumer challenge, network, exact floor, distinct observer/operator, complete reviewed request and
exact operation phase. A completed operation can be checked after its original reservation expiry
while custody and the reviewed subject remain eligible. Checks are no-write predicates. The model
cannot establish freshness of a challenge or native permission ownership: those remain the private
Core round and executed-input/result/output proof owner's obligations.

Every mutation prepares one immutable `TopologyPreparedTransitionV1` containing the exact next
summary, canonical `TopologyHistoryEntryV1`, at most one control and one operation row, and the
new signer/attester key tombstones. All counter/fence/audit rules reside in that planner. Preparation
does not mutate State; dropping a plan preserves its original owner for retry. Host indexed-read
failures remain `TopologyPreparationErrorV1::Lookup(E)`, separate from deterministic `Transition`
rejection. Core must not turn memory pressure, refund waits or unavailable storage into invalid
consensus outcomes. All semantic checks and canonical size checks precede native publication. Control
commitments exclude operation progress. The history commits the exact transition, claimed native
context and resulting heads; restart feeds that complete prefix through the same reducer.
`restore_claimed` requires the entire independently pinned retained summary and rejects missing first/middle/
last rows, duplicates, reordering, extra rows, substituted context/results and noncanonical frames.
It reconstructs audit, fence, active slot and every ID/key tombstone, then compares the complete
retained summary rather than only the terminal history hash. Its read-only inventories expose all
recovered entries for strict native persisted-index comparison. It has no snapshot decoder or
missing-row fallback. `new` may be used by Core only after proving absence of every native row/index.

Bounds are 8192 normal control revisions plus 2 emergency revocations, 65536 permanent operation IDs,
at most 2 operation revisions per ID and a 139266-entry total history. Record frames are at most 48 KiB;
history frames 64 KiB. Dynamic input vectors/identity strings are checked before canonical encoding.
These are protocol cardinality/frame ceilings, not funded memory or storage limits: the maximum
history alone permits multiple GiB. Core must admit actual nested payload/allocator, row/index,
publication, deferred cleanup and cold-replay working space before accepting work. Canonical frame
length is checked before encoding, but that check does not reserve allocations. The cold accumulator
uses ordinary uncharged maps; it must not run on each live operation. Native State owns live rows,
transaction rollback, candidate forks, snapshot completeness, drain and durable publication.

## Required next integration

- Add a topology-only ISI in `iroha_data_model/src/isi/sorafs.rs` and actual InstructionBox/visitor
  dispatch, with separate manage/operate/check permissions in `iroha_executor_data_model` and
  executor enforcement. Configure/Enroll/Revoke need manage permission; Reserve/Complete/Expire
  need operate permission; Check needs a distinct registered observer with check permission and a
  still-authorized expected operator. No bool supplied by a caller may substitute for permissions.
- Implement the Core transaction adapter under `smartcontracts/isi/sorafs_topology_authority`.
  Derive network, chain/discriminator, executing account, block height/time and topology mutation
  ordinal from the actual StateTransaction. Separately bind the true transaction/direct execution
  identity and instruction ordinal; topology mutation ordinals are not instruction coordinates. Resolve custody/floor anchors from the same authoritative
  native state cut; prohibit same-block uncommitted custody masquerading as a parent anchor. Run this
  borrowed planner over retained native indices, then atomically persist its exact history, control/operation rows and all indices under
  `sorafs_topology_authority_v1`. Do not invoke the role 14 or role 15 instruction with new labels.
- The Core history/snapshot reader must independently validate the terminal prefix and exact
  immutable rows/height indices/current heads/ID and key indices. An absent row with any retained
  index is corruption; empty defaults are forbidden. Replay must use this reducer and compare
  resulting records and metadata to persisted rows. Actual storage publication, rollback and restart
  atomicity need funded/permissioned native tests and fault tests; pure model tests do not prove them.
- Add a topology-owned prepared native Check observation. Consume the original challenge/deadline/
  finalized-floor owner and prove the exact successful ordered input, result and execution output
  under canonical revision 4 finality. Re-read permissions, custody/revocation and operation from one
  current StateQueryView before producing a non-decodable native observation. A signed arbitrary
  observer row, the model's output or an unrelated QC is insufficient.
- Register public native/schema/JSON roots and exact SDK/CLI request/receipt fixtures together with
  that actual ISI; current prerequisite types have canonical Norito framing and stable schema IDs,
  but expose no production route. Wire configured account/key/state providers only after these
  authorities exist. Reuse the existing durable operation coordinator and receipt checker.
- Replace all old 20-field detached topology-envelope producers/consumers atomically with the actual
  native-backed approval. Keep all four inner-approval gates and aggregate promotion closed until
  their genuine authority/receipt consumers are assembled and candidate-specific qualification
  passes. Software custody remains supported without an HSM prerequisite.

Tests cover borrowed/cold plan equality for every mutation, constant indexed work, exact local
refusal propagation, dropped-plan retry, summary/control/index substitution, missing current rows,
full-summary cold recovery, exact same-block timestamps, transition roundtrips/replay, candidate/request/intent binding, exclusive/idempotent
reservations, timely owner-preserving completion, revocation/rotation/renewal invalidation,
permanent IDs/key reuse, challenge/phase/floor checks, recovery after reservation expiry, bounded
capacity, malformed/missing history and failure atomicity. Counter-edge/corruption injection is
confined to the pure reducer tests and is not native state evidence. Compilation/runtime validation
of this packet and all production integration remain pending.
