# Fresh MV writer-pair acquisition

Scope: ordinary and admitted MV current/undo writer opening in the existing
`optimizations` checkout. This extends [successor acquisition](successor-acquisition-readiness.md).

The opening path formerly installed `StorageWriters` after both constructors
returned. A refusal or panic in the current constructor could therefore notify
its waiter while the earlier undo writer was still held. Pre-fix regressions
observed that ordering through nonblocking probes in the real wake callback.
The same gap existed when ordinary current acquisition encountered poison.

Both native mutexes now enter an explicit acquired phase before either cursor
constructor or policy callback. The pair transition retains both original
notifications through the complete conversion, including callee unwind. Success
transfers the same physical guards without notification; error releases both
before signaling. Both actual poison verdicts are frozen before either wake,
so a later callback panic cannot poison a healthy released sibling.

The original no-edit and clear admission kernels operate on acquired guards.
Standalone methods delegate to those kernels. Blocking ordinary opening uses
the same native acquisition and cursor constructor. Admitted opening still
reserves both writer shells and the next identity from one original pool before
acquisition, checks both acquired poison verdicts before policy callbacks, and
allocates the identity only after pair construction succeeds. Current contention
invokes no policy and releases only the actually acquired undo writer. Known
undo poison refuses immediately before probing or waiting for current; its real
release records the physical poison without inventing a current notification.

Eight MV regressions cover second-policy refusal and unwind for ordinary and
replacement modes, ordinary current poison, current contention, successful
transfer, a panicking wake with surviving waiters, and immediate undo-poison refusal despite
an independently held current writer. Native release tests cover
successful transfer plus real physical poison on refusal/cleanup unwind. The
original pool scope remains necessary for charge-refund notifications; it is
not a sandbox for arbitrary policy destructors. No second map engine or
compatibility path was introduced.

The source ledger and mutation controls bind the pair transition and concrete
constructor ordering. Both release scopes require the new regression names.
Validation artifacts belong to `dist/sumeragi-main-work/generation150-*`;
`validation150.json` records final completed checks only after assembly succeeds.

This is a local pair-construction correction. Acquisition unwind across complete
State/World/runtime owners, earlier query/advisory/persistence notifications,
aggregate resource admission, production retained Validate-to-Apply cutover and
unchanged real four/seven-validator fault/restart/final-transaction qualification
remain open. All L1–L6 remain active.
