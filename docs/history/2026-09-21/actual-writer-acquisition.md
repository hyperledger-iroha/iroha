# Actual native writer acquisition and refusal custody

Scope: the native linear-map acquisition, MV pair publication, and Core hash
publication owners on `optimizations` in `/Users/takemiyamakoto/dev/iroha`.
This continues the [joint Kura cleanup](kura-joint-release-cleanup.md), without
claiming complete production cutover or network liveness qualification.

Previously one `Changed` result covered both a foreign root rejected before
acquisition and a stale predecessor rejected after releasing its actual writer.
MV inferred a release from that error and fabricated a unit guard; Core used a
separate unit notification for the hash writer. The error value could not prove
physical ownership, and immediate notification could run under outer fences.

Native acquisition now returns one opaque owner holding the actual mutex and
unchanged private generation. Foreign-root refusal and contention acquire
nothing. Predecessor validation retains that actual owner on stale or poisoned
refusal. Successful validation moves the original writer without copying or
allocation; abort unlocks before returning its unchanged private work. The
single-owner adoption operation composes these same phases. Aggregate callers
bind their original release source before validating, rather than reconstructing
a release from an error classification.

MV returns only real acquired-release cleanup, retaining a partial pair until
all its physical writers unlock. Core keeps the actual guard through validation,
reader preparation, abort and publication. Hash refusal returns its original
notification and installation owner to the enclosing carrier or ordinary State
commit. Those callers retain cleanup until their physical fences release.
No wire format, consensus quorum, or validity rule changes.

Five new runtime cases cover foreign/busy/stale/poisoned acquisition, exact
private pointer custody, allocation-free phase transfer, unwind, partial pairs,
and reentrant hash notification and installation cleanup. Structural bindings,
the ledger and negative controls change together. Method extraction accepts a
where clause only after the exact self type; tests reject foreign owners whose
bounds merely mention the required type. Both release scopes require the new
runtime cases.

Validation artifacts belong to
`dist/sumeragi-main-work/generation148-{core,formal,release-census}`; the final
`dist/sumeragi-main-work/validation148.json` receipt records their exact scope.
Intermediate compiler, source-selector and output-capture failures are kept
separately and are not final evidence.

Remaining work includes callee acquisition unwind across other aggregate writers,
notification ordering in successor admission and query/persistence operations,
complete capacity admission, and the production retained Validate/Apply and
process-lived lane-runner cutover. All L1–L6 remain active until the same candidate
passes the required four/seven-validator faults, restart and final-transaction
qualification as well as the broader release checks.
