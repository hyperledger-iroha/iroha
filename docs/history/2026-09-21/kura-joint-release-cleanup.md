# Joint Kura release and cold lookup cleanup

Scope: the original Kura physical fences on `optimizations` in
`/Users/takemiyamakoto/dev/iroha`. This continues the
[partial-refusal cleanup](partial-publication-refusal-cleanup.md). It does not
establish complete production cutover or network liveness qualification.

Separate local guards previously notified one release while sibling Kura locks
were still held. Partial acquisition, read failure, and normal or unwinding
abandonment all shared that ordering defect. Cold pending-capacity reads also
need to unlock the sidecar before accessing the merge log, while keeping prune
and canonical custody. Retaining that physical sidecar lock would invert the
existing lock order; discarding cold accounting would change admission.

One incremental acquisition owner now retains every acquired guard. Its fixed
cleanup value exists only after all physical fences unlock. Successful leases,
partial refusal, and abandonment use that same owner. The live participant
reauthentication and archive authentication wrappers use the same aggregation.
The exact Busy observation still belongs to the lock that actually refused.

The original sidecar notification supplies a constant-space deferred batch.
Only actual guards from that same source can record releases. A foreign transfer
returns its original guard unchanged; an empty batch emits no signal. Repeated
cold reads can unlock before merge-log access without invoking callbacks under
the outer locks. Their eventual single wake is a retry hint, never authority to
publish. Physical release records poison before later cleanup can unwind.

Six Core regressions exercise partial contention, normal/unwinding abandonment,
actual persisted cold merge reads, missing/corrupt sidecars, repeated cold reads,
foreign transfer, and live custody wrappers. Callbacks attempt the real sibling
locks. Three native regressions cover source identity, empty batches, registered
and late waiters, cancellation, allocation-free repeated transfers, and physical
versus later cleanup poison. Formal declarations and the binding ledger change
together; sixteen mutations target these exact executable ownership relations.
Both maintained release scopes require the nine new runtime cases.

Validation artifacts are retained under
`dist/sumeragi-main-work/generation147-{core,formal,release-census}`. Final receipt
`dist/sumeragi-main-work/validation147.json` records executed results and exact
source/binary joins; intermediate attempts are not final evidence.

Callee acquisition unwind across other component aggregates, stale native-writer
custody, complete resource admission, and the production retained Validate/Apply
cutover still need closure. All L1–L6 goals remain active until the same candidate
also passes unchanged four/seven-validator fault, restart, and final-transaction
qualification.
