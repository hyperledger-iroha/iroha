# Stream-token native operation claim boundary

This record preserves the earlier claim and private-read slices below. The
current checkout also contains executable native Reserve, Complete and Expire
and a no-write challenged Check predicate described below. The signed Check
proof consumer, production private signing admission and token release remain
closed.

The existing role-11 private signer in
`crates/irohad/src/signer_operation/stream_token.rs` derives one operation from
the provider-scoped canonical token body and governed Ed25519 binding. It
reserves under the original verified custody and audit predecessor, performs
four ordered key operations, stages the complete private receipt before the
completion CAS, and recovers without signing again. Its state-source and key
provider tests use injected collaborators. They do not establish a production
Reserve/Complete authority.

Native provider-scoped custody already exists in
`crates/iroha_data_model/src/sorafs/stream_token_custody.rs`, the corresponding
Core instruction and bounded historical query. It governs Configure, Enroll
and Revoke, including irreversible key-generation retirement. It intentionally
contains no per-token operation, audit, reservation or completion rows. The
Torii's native finality owner authenticates custody against committed State
and Kura/QC block evidence. At this initial boundary Core's signed native
Check consumer covered only roles 14 and 15, and no retained role-11
operation journal joined a reservation to a genuine completion. Detached
observer signatures and a private daemon receipt could not fill either gap.

The new `stream_token_authority` DataModel module carries only the exact
non-authoritative claim boundary already fixed by V1 contracts: the existing
`SignerStreamTokenRequestV1` plus ordinary Sign intent, original reservation,
candidate completion commitment/signature digest/time, expiry claim and
claimed outcome. The existing Manifest request now has one strict JSON/schema
representation alongside its canonical Norito frame. Pure validators reject
changed body/binding/custody/audit expectations, wrong action, altered slot,
invalid audit successor and impossible token/reservation chronology. The
caller must derive the expected request from independently pinned canonical
token body, provider binding and original verified custody. The decoder and
validators establish claim shape only; candidate-provided completion time or
an outcome enum cannot prove execution or finality.

At that claim-only stage, no ISI or Core dispatch was added. Production remains closed until a
provider-scoped immutable operation/audit journal, first-use ID tombstones,
bounded reservation and execution-derived completion, same-State signed Check
consumer and Kura/QC finality join exist. Native custody revocation must
invalidate an active operation atomically. The daemon state source must read
those exact rows, use authenticated software custody without an HSM
prerequisite, and join the existing four-signature private service before a
token can be released. There is no compatibility layout or raw-sign path.

Focused tests cover the Manifest request strict JSON/schema/Norito shape and
DataModel operation claim canonical roundtrip, foreign coherent request, wrong
action/audit, slot substitution, false completion chronology, terminal replay
and decoder bounds. The exact offline, locked selectors passed:

```text
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p sorafs_manifest --lib stream_token_request_has_one_strict_json_schema_and_canonical_norito_shape -- --nocapture  # 1/1
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_data_model --lib stream_token_authority -- --nocapture  # 6/6, four new claim tests
```

These component passes do not authorize production Sign or release promotion.

## Provider-free completed-receipt read boundary

The role-11 daemon producer now splits out
`SignerStreamTokenCompletedReceiptCheckV1`. The checker retains configured
public custody, the original private journal's read-only lease, and a source
capability with only current-custody and exact completed-operation observations.
It has no provider, Reserve, Complete, key use, or recovery-signing method. The
signer service's existing recovery and this checker share the same canonical
receipt/signature validation, fresh AfterCommit and BeforeRelease observations,
and final journal identity recheck. An independent checker can outlive the
signer without regaining a mutation capability.

The new read-source trait is an interface, not a production finalized-state
implementation. The tests use the injected fixture source and cannot establish
native role-11 execution or Kura/QC finality. No configuration, Norito schema
or fixture changed in this slice. The production gate above remains open until
the actual native operation journal and signed Check supply this read source.

The exact offline, locked daemon selector passed three new completed-read tests:
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib
completed_stream_check -- --nocapture` (3/3). The same freshly built test
binary passed the full stream-token signer selector (21/21) and the adjacent
final-promotion signer selector (20/20), which shares the renamed read-source
trait. The latter contains two intentional caught-panic lifecycle regressions;
the suite result is passing. These local tests do not promote the fixture
source into production finality evidence.

## Typed native operation and Check handoff

The next DataModel slice defines one bounded `StreamTokenAuthorityRequestV1`
with Reserve, Complete, Expire and challenged Check actions, and a claimed
`StreamTokenNativeOperationV1` row. Reserve retains the exact reviewed body,
operation ID, custody generation and audit predecessor. Complete repeats the
original reservation/fence and staged four-signature commitment, but has no
caller-supplied completion time. The eventual native executor must derive
reservation ID, fence, execution time and the terminal result from its own
State transition. Expire keeps the ID tombstone whether timeout or custody
revocation caused terminalization. Each execution coordinate identifies the
exact transaction entry and instruction in a signed block. Structural claim
checks reject changed provider, custody, operator, reservation, audit, phase,
chronology and execution location.

Each Check binds network, registered provider, current custody revision and
digest, independently retained operator and observer, one-use challenge,
finalized floor (height, hash and signed context ID), exact original reviewed
request, and one of six closed phase predicates. The pure validator requires
the Check observer to differ from the operator. Core must additionally prove
that this observer differs from the protected signer and has the exact scoped
Check permission. The operation rows visible to Check must precede its
independently retained finalized floor. At this typed handoff stage the DTOs
were not entered into the instruction registry; decoding and pure checks
remain non-authorizing.

The production native reader must use one State view to compare the exact
indexed operation row and custody generation. It must then authenticate the
signed transaction-entry hash and entry/instruction indexes against the
canonical Kura block and QC, verify the network execution proof and successful
output, and authenticate the Check transaction itself from the independent
floor forward as the existing role-14/15 Check owner does. The success result
comes from that proof and State transition, not a caller-provided result
digest. A signed Check by itself cannot prove role-11 Reserve or Complete.
At this typed handoff stage there was no role-11 Core Execute path, native
operation row, Check consumer or production finalized read source. The first
two are now present as described below; token signing and release remain
closed.

The exact offline, locked DataModel selector passed after the transaction-entry
and finality-context additions:

```text
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_data_model --lib stream_token_authority -- --nocapture  # 10/10
```

The first run had nine passes and one test-helper panic when the adversarial
case substituted Reserve for Check; the helper was fixed and the final run
passed. No native operation execution, Check execution, four-validator test or
promotion evidence was produced by this component test.

## First executable native cut and remaining authority gap

The current source adds provider-scoped operation and separate observer Check
permissions and registers the sole native role-11 instruction with Core Execute
and dispatch. Reserve and Complete require the registered provider owner with
the operation permission. A custody manager can request Expire with its own
scoped custody permission; it does not acquire Reserve or Complete by
implication. At this initial cut, Check's observer permission was distinct,
but Check remained unavailable pending a protected-signer, finalized-floor
and same-State predicate.

The shared `StateTransaction` context retains the actual outer network
entrypoint hash and a one-shot direct role-11 instruction ordinal. Executor
sets that ordinal only around directly signed role-11 instructions and clears
it afterward. The handler consumes it at entry and requires outer entrypoint
hash equal to the inner signed transaction call hash. Thus a missing direct
origin, nested/contract-emitted instruction, or SealedReveal with distinct
outer and inner hashes fails closed. Plain signed instruction and ordered
mixed-batch positions are the intended canonical ordinals. The focused
Executor helper test checks these signed positions and closed cases. The
handler unit injects the fields manually; actual network application and
signed result evidence are still required end to end.

The Core handler uses finalized native stream-token custody and retains
purpose-owned immutable provider operation rows, a bounded 65,536-ID admission
space, permanent first-use ID tombstones, a single active slot, fencing and
adjacent digest-linked revisions. Reserve derives its reservation ID, expiry,
execution time and signed entry/instruction coordinates from the application
context. Complete and Expire compare the original retained slot. The handler
validates each proposed terminal row before writing any State path, so a
same-signed-transaction terminal attempt cannot persist a self-invalid row:
terminal provenance must identify a different later signed transaction. A
pure Expired row permits a custody manager different from the original
operator, while the Core action checks that manager's actual scoped permission.
Following custody revocation, signing fails closed and the ID tombstone
remains; a later authorized Expire may terminalize the slot. Atomic
Revoke-to-Expired in the custody handler remains open.

Reserve currently checks the submitted reviewed request against itself for
structural validity. It does not independently prepare the exact token body
and binding. Complete checks a nonzero four-signature digest and audit
successor but does not authenticate the private receipt or prove that all four
signatures were made. The private signer must independently recompute its
exact body/binding/original custody and compare the retained row, then
authenticate its staged receipt. Core Check at this cut returned
`CheckUnavailable` before any token-release authority existed. The production
same-State reader must join the indexed row to successful transaction output
and exact entry/instruction proof in Kura/QC; Check must execute and finalize
from an independently pinned floor. The daemon's injected read source is not
that production reader.

The integrated Core source compiled after the typed request's nested `Ord`
derives were added. The focused handler selector passed 2/2: it checks
direct-context consumption, outer/inner mismatch and same-transaction
terminal-row rejection before State writes. The shared output-network
source-binding selector passed 1/1. The executor signed-ordinal helper selector
passed 1/1; the helper additionally cross-checks that the typed role-11
instruction at the claimed index exactly matches the signed instruction or
mixed-batch item. The handler test injects context manually, and the helper
test is still below full network application, so these local passes do not
establish a successful authoritative execution result or finalized Check.
Four-validator finality, concurrent and restart behavior, custody rotation,
signed Check, audited private signing source and release qualification remain
open. No HSM is required for the remaining authenticated software-custody work.

## Closed signed Check binding handoff

The shared native Check proof owner now recognizes role 11 as its third closed
purpose. Its binding stage accepts only the exact one-instruction signed
External stream-token Check, original one-use challenge, observer account,
network and independently pinned height/hash/context floor. It rejects a different
signed instruction, altered floor, non-Check action, malformed signature and
cross-purpose reuse before attempting applied-State authentication. This is a
necessary handoff for a later role-11 purpose wrapper; no production caller
consumes it yet and Core still returns `CheckUnavailable` for every role-11
Check. The binding tests use a synthetic uncommitted transaction and claim
only envelope and purpose separation, not Kura/QC finality. The exact focused
Core selector `cargo test --offline --locked -p iroha_core --lib
role11_check_binding -- --nocapture` passed both new tests (2/2) after the
DataModel wire/registry selector passed 11/11.

The existing generic `authenticate_applied_check_v1` can authenticate a Check
transaction from its independent floor forward to the applied State cut,
including committee succession, signed-RS16 finality, exact External bytes,
network execution proof and aligned successful result. The role-11 Check DTO
places its floor at or after the Reserve/Complete row heights. The generic
floor-to-applied walk therefore cannot prove those earlier operation entries.
Before enabling Check, a purpose-owned verifier must authenticate the
historical Reserve and Complete rows against their exact transaction entry,
instruction index, successful network output and Kura/QC lineage into the
independently pinned floor. It must also compare the indexed row, current
custody, provider/observer permissions and phase in the same applied State
cut, then independently recompute the private request and receipt. Reusing
the generic Check success alone would omit that historical operation proof.

The existing APIs provide the necessary evidence but do not yet assemble it
for role 11. The purpose-owned read-only `read_history` now returns the exact
original Reserve and current record from one borrowed World view. It requires
an immediately adjacent terminal revision, exact predecessor digest and the
unchanged original custody revision/digest, even when the operation is buried
below a later provider head. `read_slot` retains
its latest-row interface through this stricter helper. A future verifier must
carry this pair into one `StateView` with the Check and custody state; the
helper alone establishes neither application nor finality. Each record carries
its domain-separated signed instruction/authority digest and exact execution
height, entry index,
instruction index, entrypoint hash and authority. Starting at the earliest
operation height, a bounded proof walk must compare every State block hash to
Kura's durable block and finality artifact/receipt, verify consecutive height
contexts and previous-block hashes through the independent Check floor, and
retain the target artifacts. For each target, `TrustedBlockProofAnchor` and
`network_execution_proof` must authenticate the exact External transaction,
aligned successful `network_output_at`, and the matching direct typed ISI at
the claimed plain-instruction or mixed-batch item index. The verifier must
recompute each retained request digest and compare the row's block timestamp
to its execution-derived time. The current generic Check consumer retains its
floor-to-applied proof but not these earlier target artifacts. Implementing
that history bridge and same-cut phase predicate is the smallest missing
authority seam; a decoded row or its self-selected Kura artifact cannot be
used as its own trust floor.

Four new Core history tests exercise the exact Reserve/current pair, a buried
terminal revision gap, a buried wrong predecessor and independent custody
revision/digest substitutions. Each corruption test first confirms the newer
provider head still reads coherently, then confirms the selected operation
fails closed through both `read_history` and `read_slot`. The focused Core
history selector passed 4/4; broader role-11 and shared Check selectors passed
7/7 and 16/16 on the same compiled source:

```text
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib history_pair_ -- --nocapture
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib stream_token_authority -- --nocapture
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib signer_check -- --nocapture
```

These are local State-history and signed-envelope tests. They do not supply a
production finalized Check, historical operation execution proof or authority
to sign and release a stream token.

## Bounded historical signed-execution readback

The next purpose-owned reader in
`crates/iroha_core/src/query/stream_token_authority/historical_execution.rs`
borrows one `StateView` and the exact Reserve/current history pair. It accepts
only an independently retained Check floor, not a row-selected trust start.
It checks each State block hash against Kura's signed-RS16 finality and durable
receipt, verifies adjacent height contexts and signed parent hashes up to the
exact floor hash/context, and then re-reads at most two target blocks. Each
target must have the claimed applied transaction membership, exact External
signed entry and direct instruction index, domain-separated request digest,
execution-derived block time, anchored network inclusion proof and aligned
successful output. A mixed-batch success is the single atomic transaction
result, so all preceding items must also have succeeded. Expire can name newer
current custody after revocation; it keeps the original Reserve custody in
the row and uses the signed terminal request digest plus successful native
execution instead of falsely equating the two generations.

One Check proof walk is bounded by canonical V1 limits of 4,096 heights and
64 MiB cumulative finality-artifact frames. It retains one parent artifact
and receipt and two target context IDs, with no full-block collection; Kura's
per-record bounds and the target proof anchor's block-wire bound remain in
force. The 64 MiB count does not yet bound physical I/O: the finality helper
and paired artifact/receipt read can load the same sidecar twice, and target
proof verification reloads its own evidence. Complete Check resource admission
and measured I/O remain open. Exhaustion returns `CheckUnavailable` without a State mutation. The
result is a non-serializable capability borrowing the same State view; it is
not a completed Check or token-release authority. No fixture invents a
successful role-11 block result. Source-shape, changed-custody expiry,
exact-capacity and missing-finality tests passed against the combined Core
source:

```text
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib historical_execution::tests -- --nocapture  # 4/4
```

The same freshly built Core test binary also passed the adjacent
`history_pair_` selector (4/4). The role-11 historical readback source is
frozen at this boundary; no successful Check, production daemon reader or
release authority was inferred from these tests.

The next boundary is the native no-write Check predicate described below.
Beyond it, a purpose-owned observer must use this same view for current
custody, operator/observer permission, phase and private receipt binding,
authenticate the Check transaction from the independent floor to the applied
cut, and supply genuine positive multi-validator execution/restart evidence.
The daemon's read-only source still has no production binding to that completed
Check, so signing and release remain closed. Authenticated software custody is
sufficient; no HSM prerequisite is introduced.

## No-write native challenged Check predicate

The native role-11 handler now admits a Check only as a no-write consensus
predicate. Its direct signed instruction context and provider-scoped observer
permission are checked before `apply`. The predicate bounds the canonical
request, requires a nonzero challenge, and checks the submitted floor's exact
height/hash/context against the executing State's block hashes and Kura's
durable signed-RS16 finality artifact and receipt. That establishes local
floor association, not independent floor selection: the future observer must
pin the floor before signing and use the shared Check proof consumer to
authenticate the finalized Check from that floor to one applied State cut.

On the same executing State transaction, Check re-reads the current indexed
custody revision/digest and committed parent control. It requires an active,
unrevoked enrolled signer and attester, matching enrolled record digest,
current control-state digest and canonical public software-capable binding
digest. The observer must be distinct from both the provider operator and the
protected signer; the operator must still be the registered provider owner
with operation permission, and the observer must have the exact provider
Check permission. The deterministic Check block time must be within the
token's issue/expiry interval and current policy interval.

The predicate derives each expected phase from the indexed native journal:
`Current` needs no active operation or spent ID and the exact audit head;
reserved phases need the exact active head, row digest and unexpired slot;
completed phases need the exact adjacent immutable terminal row, including
when later operations have advanced the head. A submitted row, wrong outcome
or changed audit is rejected before any State write. Check's execution-derived
time cannot precede the retained Reserve or Complete execution time even when
its block height is later. The ledger cannot
distinguish `BeforeProvider`, `AfterProvider` and `BeforeCommit` from the same
Reserved row, or `AfterCommit` from `BeforeRelease` from the same Completed
row. The later signed observer must pin the specific expected phase and
privately verify provider/receipt progress. The Current-phase body is not
independently reconstructed by Core; private preparation must bind its exact
canonical token body to the signed request.

The focused Core selector passed 4/4 against the final predicate source:

```text
CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib sorafs_stream_token_authority::check::tests -- --nocapture  # 4/4, 17,441 filtered
```

The tests include one genuinely signed three-of-four RS16/Kura floor fixture,
changed floor hash/context/challenge, software custody and binding mismatch,
revocation, wrong observer/protected signer, scoped permission, indexed phase
substitution and a no-write refusal. Synthetic operation rows in shape tests
are not claimed as finalized execution. A full successful Current-phase Check
application still needs a genuine finalized Configure/Enroll source and
parent-State/QC association; it was not fabricated for this test. The final
compiled predicate includes a test assertion consuming Kura's finality receipt
and fail-closed Reserve/Complete timestamp ordering with a negative Reserved-row
assertion. The adjacent handler selector passed 6/6 on this final compiled
Core source.

`check_floor` currently calls the shared finality verifier and then reads the
artifact/receipt, so Kura may load and verify the same sidecar twice. Neither
this predicate nor the historical 64 MiB canonical-frame count proves a whole
Check physical-I/O/work bound. The purpose-owned signed Check proof, same-cut
history/custody/phase observer, private receipt commitment, daemon finalized
read source, full positive application/restart evidence and resource admission
remain open. Native Check success alone authorizes no signing or stream-token
release.

## Torii completed-observation publication gate

The production Core-backed Torii finality validator now refuses `AfterCommit`,
`BeforeRelease`, and any `CompletedOperation` subject before reading State or
Kura. This closes the path where a signed observer's coherent completed-row
claim, combined with a certified block coordinate, could pass Torii's final
publication check without authenticated native Reserve/Complete execution and
a finalized challenged Check. Current-only startup and admission phases retain
their existing checks. A pure negative test covers both completed phases and a
completed subject substituted under a current phase; its synthetic row is only
an untrusted shape, not execution evidence.

The publication gate is reached after the signing provider call and before
the pending signature can escape as a token. A separate private readiness
guard now runs before issuer quota reservation, and is repeated immediately
before the provider call: Core refuses because the required completed-operation
proof source is absent. A test-only simulated finality source can exercise the
existing signing protocol, while forced readiness failures require zero provider
and recovery calls and leave issuer quota unchanged. This prevents a production
signer reservation or signing side effect while release evidence cannot be
checked. Same-State native operation proof, private
receipt commitment, finalized signed Check proof and daemon reader remain
open. These are fail-closed boundaries, not production signing qualification.

The final offline, locked Torii build passed both completed-proof-source tests
(2/2). The same compiled test binary passed all adjacent token issuer/signer
tests (30/30) and native finality tests (6/6). The issuer suite's intentionally
caught poisoned-lock panic is part of its passing negative test. The direct
signer test uses the configured key revision and asserts that its prepared body
is valid before forcing the proof-source refusal; therefore that refusal is
observed before provider I/O rather than hidden by a fixture mismatch.

## Next same-State observer capability

The smallest purpose-owned Core consumer starts a one-use Check round with an
independently pinned signed-RS16 floor, exact provider/operator/observer,
prepared body and phase before accepting any row. The shared signed-Check
binder must retain the one direct External instruction and its challenge.
`authenticate_applied_check_v1` then owns one applied `StateView` after proving
the signed Check, successful aligned output and floor-to-applied finality. The
role-11 wrapper must pass `cut.view()` into
`authenticate_stream_token_history_to_floor_v1` with that original floor,
then use the same view for active custody, committed control index/digest,
current revocation, registered provider ownership and both exact scoped
permissions. It must compare the Check's phase and reviewed request with the
authenticated Reserve/Complete row rather than adopting a signed candidate's
row. The role-11 instruction currently owns the permission predicate; its
read-only policy check needs one shared implementation for the observer.

The private receipt must be bounded and canonical, independently prepared from
the token body and pinned binding, and joined to the exact native request,
intent, original custody, reservation, audit/response commitment and ordered
four-signature digest. The completed observer request also pins the canonical
receipt digest. The current native `StreamTokenCheckV1` carries the completed
row's signatures digest through its phase but no exact receipt digest. The
first-release signed Check or its purpose-owned signed observer proof must bind
the same privately retained canonical receipt digest before that Check can
authorize release; no detached observer-selected receipt is sufficient. The
generic `SignerCompletedOperationV1.anchor` contains
`operation_state_digest`, but no production role-11 source currently defines
how it maps to the native journal at completion height. The immutable terminal
row digest is a possible purpose-specific mapping, but it has not been
specified or implemented; it cannot be silently equated to the latest head,
which may advance after completion or within the same block. A positive
non-serializable observer capability and daemon reader must remain closed until
that exact mapping, terminal height/hash association, private receipt binding
and two fresh challenged completed phases are implemented and tested with real
finalized execution evidence. No fixture-generated row is release authority.

Adversarial coverage must include changed floor hash/context, signed Check
rejection or absence, wrong permission or provider owner, revoked or renewed
custody, substituted phase/operation/receipt/signature digest, later journal
head advancement, same-block operations, stale original custody, restart,
missing Kura artifacts and exact resource limits. The current Check consumer
walks every floor-to-applied height without an explicit height or cumulative
frame limit and loads a canonical block at each height. The historical reader
has a 4,096-height/64 MiB finality-frame cap, but its finality checks and
target proofs may reread the same Kura evidence. Neither bound establishes a
whole observer physical-I/O or allocation admission limit. A combined
bounded, streaming proof or charged shared read path is required before
production qualification.
