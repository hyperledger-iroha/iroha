# Canonical QueuePlan admission and closed-route Queue release

The previous public QueuePlan response returned `202 Accepted` after a local
`f+1` certificate was durable, before any carrier installed its admission in
canonical State. If an autoscale route closed first, the certificate could no
longer enter a valid carrier. Its local Queue claim then blocked both the
lane-drain vote and the final physical-retirement veto, waiting for the same
progress that the veto prevented. A partial attempt without a certificate had
the same local Queue problem.

The first-release boundary now treats the quorum certificate as an internal
availability input. Torii returns the certificate with public `202` only after
the exact logical admission appears in the canonical registry. It waits under
the original ingress deadline; an uncarried durable input retains its bytes
and returns outcome-unknown on timeout. A canonical conflicting owner returns
conflict, while stale uncarried work remains indeterminate to the client.

State authenticates the committed close of the exact autoscale incarnation,
including its canonical authority context, even after a drain commitment is
installed. Queue compares each durable claim against that State snapshot. An
uncarried claim whose bound route is closed is removed only after an exact
force-synced journal tombstone. A selected, popped, or reserved owner retains
the claim until its original release; a canonical registry owner is never
terminalized by this rule. Drain voting and retirement validation invoke the
same reconciliation before their local Queue checks. Queue pop also recognizes
the closed terminal before treating the immutable old route as a corruption
fault. Startup replay tombstones the same uncarried claim before republishing
it into memory.

Focused Core library compilation passed. The live close test covers certified
and ordinary durable claims, Queue pop, journal removal and drain observation;
the canonical-membership test retains an admitted claim; the restart test
removes an uncarried claim without reopening an invalid route. All three Core
cases pass again on the final Rust source. The Torii
canonical-wait and pre-registry-response tests passed, as did the retained
handoff case. This is scoped development evidence, not a network liveness
verdict.

The reviewed QueuePlan source ledger now binds the canonical wait and its
public-response gate, plus exact-claim terminal cleanup. All 52 selected
source-contract cases pass, including mutations that remove the wait or change
its height, and the full multilane structural checker passes. These checks
bind source behavior to the model; they do not extract a production trace.
The QueuePlan TLA action and public-202 invariant now require canonical
membership. Three focused model/source cases pass on this final relation,
including both relation-removal mutations. The pinned TLC execution has not
been run for this checkpoint.

Still required: a real four-validator delayed-receipt/partial-attempt closure
campaign with restart; distinct route committees; physical retirement under
selection and storage contention; and the unchanged broader loss, reordering,
backpressure and final-transaction matrix. The remaining source and resource
ownership work in the Sumeragi redesign plan is not closed by this checkpoint.

A follow-up ownership review found that ordinary exact QueuePlan rejection
selected the unguarded path even when a selected or popped transaction still
owned the same durable claim. The public rejection entrypoint now uses the
guarded path already used for replay terminal cleanup. It keeps the exact
journal claim until selection or guard release; autonomous reservations stay
under their original Kura terminal corridor. Two Core regressions hold a popped
guard and a global selection lease across rejection, respectively. Both Core
regressions pass. The source ledger binds the guarded terminal path and the
multilane structural gate passes.

The first attempted four-peer autoscale run selected a September 23 release
daemon against September 24 test and client sources. Status decoding failed
with a Norito length mismatch before block one, so that run cannot qualify
consensus behavior. A same-source daemon rebuild and pinned rerun are required.

The follow-up simplification removes the duplicate exact-rejection cleanup
routine. Exact binding authentication now hands the original claim to the same
guarded terminalizer used after committed route closure; one reservation-phase
predicate covers live, commit, tombstoned and release custody. A separate
leader-snapshot correction skips an uncarried QueuePlan claim when its canonical
registry marker is absent. That claim and its journal remain live, while later
eligible transactions can be selected; the exact admitted and reserved barriers
remain unchanged. This follows the first-release rule that an off-chain receipt
is availability evidence, not a canonical ordering promise. The four Core
owner and selection regressions pass on the final Rust source. The multilane
structural checker, 56 replay-terminal/membership controls and 32 retained-route
controls pass on the revised bindings.
The subsequent bounded-scan correction makes the leader's local sample cursor
advance across unadmitted QueuePlan claims without scanning the entire queue
under one lock. A new committed parent or queue reorder resets the cursor;
canonical QueuePlan, selected, transitioning and durably reserved owners retain
the FIFO cut. The
one-item-scan Core case reaches a ready transaction behind three unadmitted
claims and then reconsiders a newly canonical earlier claim. Four existing
bounded-snapshot tests, the single-claim case and both held-owner rejection
tests pass on the same Rust source. The revised formal source-binding gate
passes; it does not prove a network trace.

A same-source four-peer drain test reached genesis but timed out before its
first drain action because its physical storage probe expected retired
`blocks/lane_*` directories. Preserved peer storage shows the current
`blocks/instances/<identity>` directories and durable incarnation markers on
all four peers. Retired physical instances are retained for recovery, so the
integration probe now matches each marker against the peer's committed active
catalog before counting it; a fresh integration run is required to reach
the drain/restart assertions. This harness failure is not a consensus pass.
