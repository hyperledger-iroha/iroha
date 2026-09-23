# SORA Nexus receipt directory durability

## Problem and resulting behavior

Repeated strict reads of an occupied lane receipt flushed its data file, index,
and every directory through the Kura root. A retained debug recovery fixture
sample located repeated directory flushes in State pending-height lookup,
hydration, and completion checking. The sample contains kernel waits; it does
not establish production CPU utilization or payment latency.

Strict reads now retain only original directory handles after a successful
complete namespace barrier. Reuse requires the same original sidecar-mutex
mutation epoch, exact paths/identities, and unchanged timestamps for every held
ancestor, checked around namespace revalidation. Every occupied read still
flushes its current data/index descriptors, decodes the current bytes,
authenticates current canonical execution, and performs the final exact receipt
re-read. Corrupt occupied files remain errors. No application/finality verdict
or regular-file descriptor is retained.

The original sidecar fence issues a private, non-Clone read permit at construction.
All existing ordinary, try-lock, generic lease, recovery, and mutation acquisitions
advance its epoch before caller work. Error and unwind cannot restore an earlier
epoch. Saturation permanently disables reuse. Only audited immutable paths use
the paired permit; the mixed raw reader does so only when recovery is disabled.
Other publication mutexes allocate no epoch owner or perform epoch atomics.

## Bounds and ownership

The current lane-instance constructor yields five directories: Kura root,
`blocks`, `instances`, the instance identity, and `lane_artifacts`. Twelve inline
slots retain at most 60 directory descriptors, below the 64-descriptor
admission bound. Actual retained vector and path capacities are bounded by
256 KiB, plus the fixed inline owner size. The owner contributes its retained
namespace/directory associations to the existing ResidentFrontier inventory.
Transient reader descriptors are separate from retained custody. Directory-only
custody cannot keep unlinked regular receipt payloads consuming disk space.

The actual storage guards remain ordered prune, canonical, geometry, sidecar,
then the small durability owner. No Kura guard spans a State/hydration call.
Checked production mutation routes include ordinary receipt writes, append and
temporary-file recovery, prune/recovered prune, terminal compaction, geometry
publication/startup recovery, and directory garbage collection. Nonlease garbage
collection holds geometry before its sidecar epoch acquisition through rename
and removal, excluding receipt reads throughout that interval.

## Validation status

The fresh build-9 Core image passed 75 scoped native checks, including all 14
publication-lock tests and 13 receipt tests. Its complete-consumer directory-reuse
test then failed; four remaining selected tests were not executed. The image and
source hashes stayed unchanged. The enclosing nine-package build separately
failed on other test compilation. The source-bound record, including the actual
failure, is in `dist/sumeragi-main-work/generation168-receipt-durability-review/`.

The matched measurement controls execute the same strict production reader and
signed canonical receipt 32 times in one current Core image. The control clears
only test-owned namespace custody between reads; the treatment retains it.
Both include all file barriers and canonical checks. Two warmups and eight
alternating repetitions per control are planned. No performance gain or
production happy-day result is claimed before those executions complete.

## Whole-consumer observation path

Repeated ordinary completion checks can reuse the retained directory barrier.
Historical inventory, MainOnly autonomous latest-pointer inspection, and Native
history now acquire the original exclusive sidecar mutex with its paired read
permit. Exact existing-slot recovery first authenticates the signed carrier,
finality, and exact ownership, then uses the strict raw reader. Only authenticated
absence enters the original repair path with ordinary mutation acquisition and
all first-write barriers. That path now binds and flushes every ancestor before
publishing the raw artifact: an ancestor failure leaves no new slot for a retry
to mistake for completed publication. The original writer then flushes data,
index, and immediate directory. Exact readback also rechecks the originally
bound namespace. Corruption and unresolved writer artifacts still fail.

The existing canonical recovery test supplies one shared four-validator fixture
for matched 32-cycle persistence and completion workloads. Each cycle checks the
exact receipt/certificate, State height, canonical hash, and restart guard. A
receipt-specific fault proves whether the whole consumer retains its directory
barrier; missing-slot and corrupt-carrier cases exercise the repair boundary.
The failed full-consumer test exposed one missed read-only acquisition: outbound
predecessor eligibility probes the current autonomous payload after hydration.
Its `LatestReadOnly` lookup used an ordinary sidecar acquisition and invalidated
the earlier receipt barrier despite performing no publication or recovery. It
now uses the same paired read permit. The fixed read-only branch returns before
the recovery branch, and retains bounded file reads and fresh pointer/marker
checks. A fifth direct epoch-preservation case covers this lookup; the original
full-consumer fault assertion remains unchanged. Native retesting is pending.

Thirty scoped Python source-binding controls passed, including the second Native
history acquisition, paired-permit bindings, ancestor-before-publication ordering,
and rejection of a mutating autonomous lookup, recovery-mode substitution, or
weakened pointer recheck. This is static evidence.

The strict-receipt comparison alone cannot establish full hydration or payment
performance. Both comparison scopes remain unmeasured on this candidate; no
speedup or current-source happy-day result is claimed.
