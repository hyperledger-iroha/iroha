# Retired-height consensus ingress and observer pressure

This is local development evidence from `/Users/takemiyamakoto/dev/iroha` on
`optimizations`. L1–L6 and release qualification remain open.

## Production failure and correction

The four-validator same-subject locked-reproposal test retained an authenticated
height-2 CommitCertificate while the peers finalized height 2 and activated
height 3. Releasing that exact old envelope made Core return generic
`WrongHeightContext` rejection. The daemon translated rejection into a fatal
downstream-delivery failure and peers exited even though the certificate's
height was already final. The retained red run is
`dist/sumeragi-main-work/generation250-message-control-network/same-subject-accepted-114-local1`.

`FairV2Ingress::try_push_owned_at` now classifies a structurally valid productive
message below the active finalized-height boundary as `Stale` before current
roster and capacity checks. This also covers a former validator after roster
rotation. It preserves the exact inbound owner and authenticated reply route
until the ingress lock drops. Active and future wrong-context messages still
reject; malformed protocol shape and fail-stop behavior remain distinct. The
daemon maps only `Stale` to the nonfatal retired relay outcome.

The Core tests cover old height, former validator, exact owner/reply retention,
and active/future rejection. The daemon test covers stale retirement without
weakening the active rejection outcome.

## Real-network evidence

The controlled daemon and ordinary integration harness were built from matching
captured Rust/manifest/fixture inputs on `optimizations`; each network runner
requires one real-network start attempt and retains peer stores and logs.

- `same-subject-final-local2` passes the four-validator same-subject
  locked-reproposal case (77.23 seconds, one test passed).
- `distinct-subject-admission-local3` passes the four-validator distinct
  PrepareQC case (196.08 seconds, one test passed). The committed height-2
  QueuePlan carries the exact complete admitted transaction as an attachment;
  its account instruction executes in the successor. The test checks both the
  committed attachment and eventual account visibility.
- The ignored nine-peer signed-observer pressure case remains under correction.
  The first run exposed a fixed-view startup race; the next showed that all
  validators had locally timed out but no timeout certificate could form while
  authenticated remote votes were held. Moving absence queries before pausing
  slow-reader relays removed a signed-query timeout. The latest retained run,
  `observer-pressure-released-local4`, delivered exactly two distinct held
  timeout votes to each of four validators (controller revision 2, zero fatal or
  overflow). All four validators durably stored height 2 while five paused
  observers remained at height 1. One `/status` request returned a transient
  503 and the test's single-read witness failed before observer recovery was
  measured. The witness now retries status errors within its original deadline;
  the final exact-source rerun is still required.

The focused Core admission test, 48 fair-ingress controls and 40 leader-wire
controls pass on the captured production candidate. The canonical multilane
structural gate passes on the latest complete source before the status-witness
change. Formatting and scoped diff checks pass. These checks establish neither
general liveness nor clean release readiness. The full four/seven-validator
loss, reordering, backpressure, restart and final-transaction campaign remains
open, as do two older NPoS fail-stops with unattributed initiating errors.

At the time of the next build, another operation started an unmerged repository
merge and changed unrelated Rust inputs. That attempt is intentionally excluded
from exact-source qualification; the merge conflicts must clear before the
status-witness build and nine-peer rerun can be trusted.
