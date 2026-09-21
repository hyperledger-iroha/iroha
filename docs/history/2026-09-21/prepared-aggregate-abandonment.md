# Prepared runtime and trigger abandonment

Fully prepared runtime and TriggerSet aggregates now keep their original physical
components in one consuming owner. Explicit abort/publication take that owner;
ordinary Drop and unwind first abort every original component, then destroy the
original journals and deferred callbacks. Capture and installation capacity stay
owned through all payload and notification cleanup. This adds no clone, replacement
journal, allocation, compatibility path or second retry authority.

Runtime controls arm a real waiter at each of the four components, and trigger
controls cover each of the ten. On normal Drop and unwind, the callback independently
probes every sibling and verifies that both resource guards remain live. An earlier
poisoned component cannot hide a still-held later component. Existing tests retain
exact original retry allocations and current/undo publication assertions. The
formal ledger and mutation controls cover ordinary raw Drop, early notification
and early capacity destruction in both aggregates.

Validation is scoped by `dist/sumeragi-main-work/validation145.json`. These controls
cover an already prepared aggregate; a panic inside an acquisition can still clean
up in the callee before the aggregate receives it. That needs incremental ownership
through the real acquiring call and enclosing State/Queue/Kura fences. Earlier
partial/advisory probes, native stale-map acquisition, successor acquisitions,
complete State effect preparation, configured admission and production retained
Validate/Apply/native runner cutover remain open. All L1-L6 remain active; unchanged
four/seven-validator fault/restart/final-transaction qualification is still required.
