# Joint Storage admission and publication

Checkpoint127 retains the original Storage current/undo owners across one admitted
insertion and prevents user cleanup from interrupting pair publication. Work is
confined to `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`. L1–L6 remain
open; this does not activate complete funded State execution or qualify a release.

## One reservation before changing any edit owner

The same Storage, Block, Transaction, View and detached-publication types carry
an explicit sealed map mode. Prepaid mode exposes checked construction, original
pair acquisition, checked transactions and closed insertion. It has no ordinary
insert/remove/mutable-value escape. The option-valued undo policy delegates real
payload copying to the same original provider. `Some(None)` remains the original
absence and is never confused with an unrecorded first preimage.

The held transaction plans both tree edits, first undo key/value copies, and
new touched-key storage before reserving their checked sum once. Component
reservations partition that original pool owner without another admission CAS or
payload allocation. Every tree executor replans under its original exclusive
borrow and checks the exact component demand. Capacity/planning refusal returns
the unchanged original inputs before policy construction, cloning or mutation.

Touched keys own a sorted initialized-prefix buffer and its actual layout charge.
Growth funds the complete replacement while the previous buffer is still live,
then moves keys and installs their new owner before dropping the old charge.
Unwind cleanup relinquishes each initialized slot before invoking its destructor;
physical buffer deallocation precedes refund. This gives explicit heap custody,
not a CPU work bound: sorted insertion shifts keys. Its worst-case transaction
work still needs configured admission/measurement before production activation.

Block acquisition holds both original writer locks while reserving the combined
cursor/reader shells and actual prior-undo clear demand. Clearing counts and
retires the actual tree without allocating a traversal stack. Busy returns the
original writer's release observation. The entire physical writer lifetime must
remain inside the budget's synchronous refund-notification scope; no caller may
wait for capacity while retaining an enclosing writer.
Ordinary planning/capacity refusal can also release a temporarily acquired raw
writer before a wrapper exists. Those releases notify the original waiters; pure
Busy/Poisoned results acquire nothing and must not invent a release.
Reacquisition that rejects a changed original base also signals its temporary
writer release, preserving the original retry observation.

## All participants before arbitrary cleanup

Private apply transfers both original checkpoints without destroying displaced
bookkeeping. It updates the parent dirty flag while the parent's failed state is
armed, then releases the original retired buffers. A cleanup panic leaves that
parent unable to read, detach or publish the partially resolved execution.
Touch-key destruction still runs before either rollback guard resolves.

Final publication has explicit prepare, publish and release ownership stages.
Preparation acquires/checks both original reader locks, writer/base identities
and healthy cursors before any node transfer. Publication installs both maps
while retaining their physical guards and all cursor/base/charge cleanup. The
MV pair identity rotates before either map unlocks. Only after both unlock and
the identity lock releases may retirement and release callbacks run. Ordinary
Block commit and retained PreparedPublication use this same pair coordinator.
A cleanup panic may report failure after the complete pair was published; it
cannot expose a current-only generation with the old undo or old pair identity.
Release notification poisoning is disarmed only after physical unlock succeeds.
A later retirement panic still notifies waiters, but cannot label a healthy map
as poisoned; later contention retains its actual release-driven retry.
Direct `Storage::insert` uses the same stages: it preserves undo, rotates the
current pair identity before unlocking, and retires old values after the identity
lock releases. The obsolete callback-only publication wrappers are removed.

The same publication stages now cover EBR-backed Cells, including current-only
replacement that preserves undo. Exclusive writer custody protects the atomic
current load and swap without invoking the epoch collector. Original unlinked
allocations remain in opaque retirement owners until every physical writer and
the pair identity lock are released; only then are they scheduled for reclamation
after the original reader grace period. Reentrant release probes cover ordinary,
prepared, untouched and current-only publication.

## Validation scope

The exact candidate receipts are under `dist/sumeragi-main-work/generation127g-final/`;
current vendor receipts are under `generation127e-vendor/`, while earlier vendor
receipts and intermediate failures remain under `generation127/`.
The final acceptance selection is 217 MV tests (151 library, 26 admitted custody,
13 linear, 5 EBR, 22 original generations), ordinary and native ASAN default/skinny;
strict MV Clippy; complete Core unit compilation and the 156 exact checkpoint126
runtime selectors plus the corrected public-contract fee control and all 14 original
geometry preparation/recovery controls; public capability compile/runtime controls; canonical formal
bindings and the pending-membership ledger; Parliament source controls; formatting,
archive, codec and diff guards. Receipt success requires unchanged complete inputs,
original artifacts, HEAD and index, not a reused success from an earlier source.

New behavioral controls cover a whole demand one byte above available capacity,
original-input retry, exact allocation/free witnesses, descending multi-level tree
splits, original absence through siblings, old-reader retention, touched-buffer
and key destruction panics, both checkpoint retirement halves, and direct/prepared
publication cleanup and reentrant writer notifications. Ordinary Clone is forbidden
by the nested-payload fixture, proving admitted copies use their original policy.
The complete Core test build also found the newer public-contract fee fixture
calling the removed `SignedBlock::errors()` API. It now uses canonical failed
outputs, checks both input/output counts and preserves every fee/state assertion.
Six geometry retry tests still passed a removed third argument to `resume_under`;
they now use the original backend/lease API, preserving their assertions and the
separate retained-custody requirement at completion.
The initial canonical gate exposed source-binding drift for newer retained Queue
custody and descriptor admission; exact executable ownership must remain matched
by their ledger and adversarial controls. Initial fixture failures incorrectly assumed a one-slot first-seen array; its actual
planned three-slot array was recorded and the regression now selects that original
current-component allocation. The diagnostic fixture's held-mutex assertion also
caused a cleanup abort; both logs remain preserved, with no production workaround.
The final parallel audit found that carrying writer-poisoning state into retained
post-publication cleanup could turn a later healthy-lock contention into permanent
poison refusal. The release phase now disarms that hint after successful unlock;
direct/prepared publication tests exercise subsequent contention and recovery.
The earlier 127c candidate passed all 171 selected Core tests and the canonical
gate before this correction; those receipts remain historical, not current proof.

## Remaining production boundaries

Native mutex/runtime and MV identity/notification construction, boxed read
iterators, concrete model payload policies, closed removal and mutation, checked
generation refusal through State, all World/IVM execution allocations and work,
and configured aggregate Validate-to-Apply admission remain required. Production
State still uses the explicit Untracked mode. No fallback from a refused prepaid
operation enters that mode. Full workspace runtime and unchanged four/seven-peer
loss/reordering/backpressure/leader-failure/restart/final-transaction qualification
remain outstanding.
