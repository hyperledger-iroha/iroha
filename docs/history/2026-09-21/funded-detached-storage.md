# Funded detached Storage and release ordering

Prepaid Storage could execute and publish inside one callback, but could not
return its original funded current/undo journals for later Validate-to-Apply.
Using the untracked capture API would lose that allocation contract; keeping
physical writers alive across the asynchronous handoff would block other work.

`try_capture_admitted_block` now uses the original admitted opening, replacement
and edit engine and moves those same journals, their successor identity and the
callback result into `Detached`. Capture does not allocate or clone payloads.
A callback error abandons the private pair; a caught edit panic cannot become a
usable journal. Original node and identity charges survive source destruction.

Reattachment uses `try_prepare_admitted` and a borrowed `AllocationScope` from
the original pool. The higher-ranked synchronous callback and thread-bound token
prevent physical prepared owners from escaping the refund scope. Same-pool
budget clones are accepted; an equal-sized foreign pool is rejected. Multiple
pairs can be held in one scope, so refunds cannot wake while a sibling prepared
pair still holds its writers. Busy, changed, poisoned and foreign-owner refusals
return the original journal. Abort returns that same detached owner.

The shared capture/abort engine also fixes native release-notification ordering.
It previously signaled after releasing the first writer, while the second was
still held. Both physical writers now release before either signal. A callback
which unwinds after release cannot falsely poison a healthy writer. This applies
to production untracked Storage capture and abort as well as prepaid Storage.
It does not claim all aggregate State destruction or publication is now prepared.

Six new runtime regressions exercise full-capacity capture/abort/publication
without allocation or copying; original-owner retry after wrong-pool, foreign
and busy refusal; replacement and stale-pair rejection; joint pool wake ordering
through publication, abort and unwind; caught edit panic; and native callback
reentry/unwind after both writers release. Four new doctests include a positive
abort example and compile-time rejection of escaping or cross-thread scopes.

Both complete MV layouts pass 278 runtime tests and five doctests without
compiler diagnostics. Exact source, executable inventories and executed names
are joined to 7,275 unchanged compiler inputs on `optimizations`, based on
`36f7cbf27661a46ccba2cf8e4bae53d321ecf1d2`. All 272 baseline runtime cases remain.
The release harness passes 220 Python tests and 2,678 subtests; all 262 selected
ownership regressions are required. These are component checks, not network
qualification. The initial test-fixture compilation failure remains separately
captured; its fixed tests preserve the original assertions.

Core library test compilation passes without diagnostics. All 58 original
review controls and 106 affected State journal/hash/physical/Native publication
consumers pass on that captured binary. The default shipping Core library check
passes with the same eight existing warning messages. The canonical structural
preflight passes; checkpoint137's final local receipt records the exact final
source gate, original pending-membership ledger and changed-binding controls.
The broad Native preparation mutation run was intentionally interrupted and is
not passing evidence; the focused selection checks current owners, exact ledger
tokens and rejection of a substituted constructor refund pool.

World still uses untracked maps. Concrete model policies, native lock/release
and runtime storage, decoding, aggregate execution/restore admission and the
production retained validator remain open. The existing pair publisher still
acquires active-reader and visibility locks during publication; finishing all
aggregate physical preparation before the first transfer remains a separate
required boundary. Native release ordering for general abandonment and joint
State cleanup also requires qualification. This change does not activate Native
ingress, retire scalar Validate/Apply, or close L1–L6. Full workspace and one
unchanged real four/seven-validator fault/restart/final-transaction candidate
remain required.
