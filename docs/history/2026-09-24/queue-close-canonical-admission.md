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
guard and a global selection lease across rejection, respectively. The source
ledger requires the guarded call, the multilane structural gate passes, and
Rust runtime validation of these new cases remains pending.

The first attempted four-peer autoscale run selected a September 23 release
daemon against September 24 test and client sources. Status decoding failed
with a Norito length mismatch before block one, so that run cannot qualify
consensus behavior. A same-source daemon rebuild and pinned rerun are required.
