# Native reader-mutex readiness

BlockHashes publication used a writer-release observation when the native
active-reader mutex refused preparation. After acquiring its writer, a refused
attempt released that same writer and immediately satisfied its own wait. Ordinary
hash reads also emitted synthetic writer notifications. Neither signal identified
the physical owner whose release made preparation possible.

Concread now owns the single release-notification implementation and an observation
for its actual active-reader mutex. Every acquisition wraps that mutex; reads,
advisory generation comparisons, preparation and publication notify through the
same source. MV and Core import the lower owner's types directly, with no duplicate
engine or compatibility aliases. BlockHashes observes the reader source before
each reader probe and the writer source before writer acquisition. Refusal keeps
the original journal. Releasing its own writer, a pinned snapshot or another map
cannot satisfy a reader-mutex wait.

Published native commits release both physical mutexes before returning cleanup.
Their retirement owner retains the original notification state without allocating
a replacement source. Dropping retirement delivers the release after the caller's
enclosing fences are gone. A later cleanup panic cannot poison a mutex that was
released normally. This preserves the existing separation between physical
publication and user cleanup.

Regressions cover contention before registration, release before first poll,
foreign sources, original-journal retry, both commit paths, preparation abort,
physical release before notification, and wake/cleanup unwind. The nineteen
existing release tests retain their bodies across eleven native primitive tests
and eight MV adapter tests. The default native suite contains two existing
split-off stress tests excluded by the `skinny` feature; both feature inventories
are checked explicitly. The formal ledger binds each BlockHashes probe to its
actual source, with mutations rejecting writer-based reader waits and self-wakes.

Scoped captures are under `dist/sumeragi-main-work/generation140-core`,
`generation140-release-census` and `generation140-formal`; the final
`validation140.json` records the completed commands and unchanged input joins.
Earlier captures retain the test relocation compile error, stale harness ordering,
feature-specific count mismatch and unused test import. These are separate from
the final passing results and are not erased or treated as qualification.

Whole-State preparation must still acquire every fallible physical lock before
the first field transfer. Explicit preparation abort, default drop and short
advisory operations need complete aggregate notification custody. Concrete World
payload/native control storage, aggregate admission, production retained
Validate-to-Apply, full workspace and unchanged real four/seven-validator fault,
restart and final-transaction qualification remain open. L1–L6 remain active.
