# State acquisition and executing-block custody

The checkpoint155 regressions reproduce a release-order defect before prepared
carrier capture. A pristine-stage refusal and ordinary `StateBlock` abandonment
both notify a World waiter while the original membership writer remains locked.
The callback performs a nonblocking acquisition of the actual membership writer;
the two original failing runs are retained under
`dist/sumeragi-main-work/generation155-core/red-runtime`.

Canonical acquisition now retains World, membership and the four runtime Cell
slots in one partial owner before initializing the Cells. Generation retry and
unwind release all acquired physical writers before payload cleanup or native
notifications. Successful acquisition transfers those same originals into an
armed result. It does not reconstruct journal payloads or add a compatibility
constructor.

Ordinary, scratch and replacement State construction share one kernel. It
prepares metadata while the acquired owner remains armed, then moves the exact
fields into an armed executing `StateBlock` before invoking its continuation.
The existing startup-hook and replacement DA-rewind order is retained. The
executing owner releases all sibling writers on abandonment; `PreparedCarrier`
delegates to that owner instead of repeating its release inventory.

The regressions also cover pristine-stage panic and dropping the completed
acquisition before constructing a `StateBlock`. They preserve original journal
identity, unchanged committed snapshots and native mutex poison. All four are
required by both release-check scopes. Torii governance fixtures use the existing
explicit World-only fixture commit API instead of destructuring the executing
owner. Their governance assertions remain unchanged.

The expanded autoscale regression selection exposed fourteen fixture failures
that also reproduce in the preserved checkpoint154 binary. The shared commit
fixture now stages the actual carrier hash alongside transaction membership.
Custom-catalog fixtures establish their intended immutable physical baseline
when constructing State; the restricted-profile case still requires its exact
policy rejection. No failing cases or assertions are removed from qualification.

The Parliament source checker follows the defining shared constructor module
and verifies transfer into the armed State owner before its continuation. Its
original mutation cases remain required alongside constructor-specific controls.

## Qualification boundary

Current build, runtime and formal results belong to the generation155 evidence
directories. Development failures remain recorded separately from final passing
runs; a build or source-contract check alone does not establish runtime progress.
The checkpoint154 receipt remains evidence for its earlier source only.

Consuming commit after the executing owner is split into fields, complete State
effect-lock preparation, resource admission and the retained production
Validate/Apply cutover remain open. Real unchanged four/seven-validator
loss/reordering/backpressure/leader-failure/restart/final-transaction qualification
is still required. This correction does not close L1–L6 or establish release
readiness.
