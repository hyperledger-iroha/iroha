# Completed repair after an interrupted manifest write

Work remains in `/Users/takemiyamakoto/dev/iroha` on `optimizations`.
The original reviewed complete-temporary recovery is extended to the earlier
physical crash cuts: exclusive creation and partial write. The durable original
`CompletedRepair` index and completed receipt/latest pointer can survive while
the manifest temporary is empty or contains only a prefix. Rejecting that file
before authenticating its repair owner made both Strict restart and live retry
permanently fail on recoverable state.

## Recovery boundary

Startup and both live repair publication paths now use one recovery kernel before
the ordinary strict inventory and route preflight. Only an existing authenticated
`CompletedRepair` locator grants this recovery authority. Canonical selected full
wire, exact merge association, signed finality, finalized WSV join, journal-bound
physical target, original stable receipt and exact latest pointer independently
reconstruct the expected manifest. Temporary bytes do not establish authority.

A missing stable manifest and an exact proper prefix of the reconstructed frame,
including zero bytes, permit removal of that one temporary. All per-file, record
count and aggregate bounds include the original prefix before the logical retry
plan omits it. Every selected carrier and its routes pass authentication before
any deletion. The original no-follow file descriptor, physical identity and
namespace remain retained and are rechecked through exact-object unlink and
parent-directory synchronization. Physical accounting records the removal.

Complete canonical temporaries keep the existing promotion path. Foreign,
non-prefix, wrong-height and unindexed temporaries remain errors. Fresh admission
uses the unchanged strict inventory. Prefix cleanup retains the original repair
locator and full missing-manifest reservation; actual repair publishes the full
manifest and retires the locator. Receipt and latest-pointer bytes are unchanged.
The later startup audit still checks unrelated physical routes; this is not a
cross-store rollback guarantee for an unrelated failure.

## Regression and qualification scope

The actual writer has a test-only cut after exclusive creation or after writing
half of its original buffer, before completion. Three added controls exercise
empty/partial Strict restart, live retry, and refusal without deletion for a
later invalid route, absent locator or incorrect prefix. They preserve exact
filesystem bytes, original locator, reservation and repeated-completion checks.
The existing complete-fsync, foreign, tampered, wrong-height and unowned controls
remain selected, including the assertion that complete-temp capacity rebuilding
does not mutate any file.

Generation158 Core test compilation passes without compiler warnings or errors.
All 887 selected Core controls pass on the same executable and unchanged Rust
inputs: the preceding 884 controls plus the three added crash regressions. The
separate focused run passes all 11 completed-repair controls, and all ten MV
documentation tests pass. The seven-package dependent test-target check passes
with 127 warnings and no errors. Torii compilation and all 21 selected runtime
controls pass. Independent verification authenticates these Rust scopes. The
266-control copied formal suite and three source-inventory guards pass under
`dist/sumeragi-main-work/generation158-core/` and `generation158-formal/`.

The initial copied formal suite retains 246 passes, nine failures and eleven
fixture errors. The reviewed Kura include inventory omitted the new module; two
providers were also absent from the copied test closure. The exact include and
its authenticated inventory digest were aligned, and the same 266 controls
pass with 3,392 unchanged copied source inputs. Their raw runner reports live
source/HEAD drift because the next candidate and an external commit appeared
during the copied run; that is not a current-source qualification. The subsequent
indexed-publication candidate will receive the final canonical structural gate; no generation158
canonical-gate pass is claimed. A prior capture setup copied a nested
generated target directory and exhausted disk; only that newly created target
copy was removed, with the setup failure retained. Original artifacts were not
removed. Git staging changed during Core runtime while Rust input hashes,
branch and HEAD remained identical; the observed staging is preserved and is
recorded separately from compiler identity. This task did not stage or commit.

## Adjacent first-publication gap

A subsequent audit found the same earlier crash cuts during initial
`CanonicalWrite` publication. Its original durable index can coexist with a
partial manifest, receipt or latest pointer before WSV application. The current
completed-repair recovery does not authorize those cuts. Startup also rebuilds
an eligible latest pointer before Apply while retaining its physical allocation
until the later WSV cleanup, counting those published bytes twice.

The subsequent [generation159 correction](native-indexed-publication-recovery.md)
uses a shared indexed-publication recovery kernel, distinct authority checks for
the two origins, actual writer cuts for all three artifacts and physical
component consumption independent of application completion. It also recovers
authenticated completed-pair latest maintenance after index retirement. Its
current qualification is recorded separately; the generation158 results above
do not qualify that later candidate.

The preceding [membership prerequisites](membership-allocation-prerequisites.md)
retain their own exact captured lower-library validation and corrected 884-control
Core interval. This correction does not close full membership funding/restore,
retained Validate-to-Apply production cutover, full workspace testing or unchanged
four/seven-validator fault/restart/final-transaction qualification. L1–L6 remain
open.
