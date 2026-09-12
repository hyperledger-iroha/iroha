# Mochi canonical SDK stream checkpoint

This is scoped local development evidence, not first-release qualification.
The candidate preserves the previously qualified finite Native AMX settlement
and hash-only schema traversal; it adds no stack override or protocol bypass.

## Implementation

- Supervisor constructs an immutable account reader from the validated selected
  generation, exact genesis authority, network identity and peer endpoint.
- Mochi uses the canonical SDK block/event capabilities; its raw socket API and
  intermediate raw-frame broadcast are removed. UI fanout retains the first
  receiver, actual received binary sizes, typed failures and cancellation.
- Readiness preserves exact signed submission bytes and the existing deadline
  and reconciliation policy. Only HTTP 429 stream rejection permits the HTTP
  reconciliation fallback; 401, 403 and 503 stop before submission.
- SDK HTTP failures preserve a single valid Retry-After delta. This metadata
  never triggers automatic replay. Transport/decode errors retain structured
  source data; an unavailable received size remains absent.
- Mock stream authorization checks exact canonical signatures, authority,
  network, route, freshness and nonce replay. Mock acceptance does not establish
  an on-chain grant or four-validator consensus behavior.
- Desktop startup passes parsed configuration directly into each application;
  the process-global CLI override registry is removed. Tests use explicit app
  contexts and one shared guard for process-environment readers and writers.
- The Kagami signing test adapter returns a verified block/manifest pair and
  publishes the bound manifest only after the signed block. Both fixture
  generators honor the exact supplied VRF seed and public authority parameters.

## Qualification

The final combined Mochi build succeeded with 179 unchanged scoped inputs
(fingerprint `b60d3be92aef5e668825cf717dc3510fd9be4338bcbbecfe7eb939b81f8d1491`).
Its exact compiler artifacts pass 445 core tests (one ignored), 181 GUI tests,
12 readiness tests (one ignored), nine mock-Kagami tests, three integration
library tests (one ignored) and two stream integration tests. The explicitly
selected real-Kagami bootstrap test also passes, including the sole reader grant
and every existing Hijiri assertion. All runs use the default test thread stack.

A concurrent merge changed Cargo.lock and two SDK inputs after compilation.
The core and GUI runtime wrappers correctly rejected source drift even though
the test executables passed; readiness's runtime source was stable but differed
from the compiler source. These are artifact results, not merged-tree passes.
All 179 exact compiler inputs were recovered and fingerprint-verified in
`as-built-inputs`; three drifted files came from matching retained Git blobs.
The remaining artifact runs verify that snapshot and executable hashes before
and after execution. The separately built real Kagami has 68 unchanged direct
Kagami/genesis inputs, not a complete transitive release-source capture.

The preceding SDK candidate passes all 783 library tests and six fresh doctests;
its strict library Clippy check passes. Eleven selected SDK/consumer packages
compile with all targets on the earlier scoped source. Those results precede
the final Mochi fixture repairs and concurrent merge; they do not qualify the
merged SDK. Five manifest scopes, 16 feature-resolved dependency boundaries,
53 architecture script tests, codec and history guards pass on their recorded
inputs. The source budget still reports 238 findings with 172 exceptions.
Mochi strict Clippy remains a nonpass: the duplicate local error-envelope derive
emits two unexpected-cfg diagnostics and genesis construction takes eight
arguments. No lint suppression or policy exception was added.

## Limits and retained failures

The first full core run found two independent failures: the stale Kagami
fixture omitted a mandatory mint-finality field, and copied ingress defaults
had drifted from node configuration. The poisoned environment mutex caused
77 follow-on failures; three tests failed independently from those two root
causes. Production validation was not relaxed; the fixture uses canonical typed
construction and the supervisor's provided authority parameters. The first typed
fixture also rejected its noncanonical shell placeholder; it substitutes a uniquely
matched serialized chain value after constructing a valid typed manifest.
The initial GUI run exposed the same ingress-default drift on seven validators
(165 passed, 14 failed including 13 lock-poison follow-ons). Mochi now depends
on the configuration owner directly and uses its defaults and checked arithmetic;
copying the three literals and the duplicate helper has been removed.

The next core run passed 383 tests and failed 60: a missing bound-manifest
fingerprint caused one primary failure and 59 poisoned-lock follow-ons. The
fixture also ignored an explicit VRF seed; fixing seed materialization alone
left the independent fingerprint failure (12 passed, five failed in the focused
genesis suite). Both failed source captures remain retained. The signing test
adapter now publishes matching signed-block and bound-manifest artifacts; the
production consistency check remains mandatory.

The next GUI run passed 175 tests and failed four: its maintenance test exhausted
a sleep/poll loop before the real worker completed, followed by three poisoned
environment locks. The tests now await the real completion message with a bounded
diagnostic watchdog and pass that same owned result to the unchanged GUI handler.
All storage, metadata and Kagami-invocation assertions remain.

The next complete core run passed 392 tests and failed 51: the restart test
assumed its shell child would exit within 10 ms, with 50 lock-poison follow-ons.
It now waits for that actual child's failure before exercising the unchanged
restart timer and manual-stop assertions. The next GUI run passed 176 tests and
failed three during initialization; restore passed alone on the same executable.
The review identified retained temporary paths in global CLI configuration and
inconsistent synchronization of process-environment readers and writers.

The earlier all-target check of all 11 selected SDK, Mochi and other consumer
packages passed on its captured source.
The external SDK error-constructor audit found one test-network literal requiring
the new optional Retry-After field; its existing classifier assertions remain.

A subsequent readiness run passed 11 tests and failed one (one real-Kagami case
ignored): a port fixture assumed that eight neighboring OS ports were free.
The allocator correctly skipped an occupied port. Exact contiguous order,
requested cursor starts, collision handling and actual `u16` wrap are now tested
through the real cursor and shared-reservation algorithm with controlled
availability; the socket integration holds a busy listener throughout allocation
and checks eight unique nonzero assignments avoiding that listener.

Initial compile diagnostics (chain parsing, policy construction and an incorrect
package name), the SDK helper visibility lint, direct included-file formatting
and the first failing manifest-budget report remain in the checkpoint artifacts.

All original 146 Torii tests remain covered; one duplicate block failure case
became an event failure case while the block check remains. All 22 fixture files
are byte-identical. The independent review records each changed assertion.

The dependency policy changes only the three required Mochi edges plus the
previously committed SoraFS rustix edge and reviewed manifest fingerprint. All
SDK/model bounds, forbidden layers and model optimization settings stay fixed.

Full workspace, mandatory four-validator scenarios, native/device delivery and
pinned baseline/candidate memory measurements remain unqualified. The existing
source-file violations remain outstanding. This checkpoint does not complete
the architecture redesign goal.

Exact dirty beforeimages, patches, compiler-artifact identities, scoped input
fingerprints, raw logs and reviews are retained under
`target/architecture-redesign/mochi-sdk-streams/` (ignored local evidence).
