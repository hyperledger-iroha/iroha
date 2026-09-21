# Prepared carrier abort ownership

Checkpoint143 carries original abort notifications and installation reservations
out of MV current/undo pairs alongside their original detached journals. The
prepaid wrapper borrows the original allocation scope through cleanup; a negative
documentation test checks that the notification owner cannot escape it.

TriggerSet and runtime abort collect every component's cleanup before returning.
World stores aborted cleanup in its original prepared boxes, transfers original
journal boxes back to the original retry vector and retains the prepared vector
until aggregate retirement. A vector owner releases every prepared World field
before its field destructors run, including preparation or publication unwind.
A panic during the subsequent retry-box transfer does not poison locks that have
already been released. Installation and capture reservations outlive those boxes.

Membership abort and block-history abort now return their original notifications.
The complete acquired carrier owns all participants through one consuming wrapper.
Explicit abort and wrapper Drop both release World, runtime, membership and history,
then all enclosing State/Kura/Queue fences, before cleanup or journal destruction.
Original storage identities, boxes, vectors and source journals are preserved;
this change introduces no replacement journal or rollback allocation.

Regressions exercise original current/undo images, two-owner callback deferral,
all World field writers and shell layouts, the exact hash release source, and a
component wake after complete carrier abort, ordinary drop or panic unwinding.
The membership and Native source bindings and mutation controls track the new
owners. The scoped receipt is `dist/sumeragi-main-work/validation143.json`;
intermediate build/test attempts remain separately recorded in generation143.

This covers a fully acquired carrier and World-local abandonment. Earlier
partial acquisition, successful advisory probes and successor acquisitions can
still release notifications while enclosing participants remain held. Standalone
TriggerSet/runtime Drop and callback propagation from their failed preparation
also remain separate work. Membership history still needs complete shard/resource
preparation; State effects, production admission and retained Validate/Apply/native
runner cutover remain unfinished. No L1–L6 or real four/seven-validator qualification
is claimed from these component controls.
