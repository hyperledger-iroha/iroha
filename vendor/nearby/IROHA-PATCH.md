# Nearby Swift package ownership

This package retains Google's Nearby revision
`2cd9371cd0aab2d3ea1c2e74c92563288ea10784` and its exact pinned JSON,
Protocol Buffers, SMHasher and UKEY2 submodule revisions in `UPSTREAM.json`.
All 356 compile and test sources from the upstream SwiftPM target description,
plus their declared header trees, transitively included headers, protobuf inputs
and licenses, are included.
Every retained source is bound by Git blob identity and SHA-256. The two
reviewed timestamp patches below preserve the original upstream identities
alongside their separate `local_patch` digests; all other sources match upstream.
The unmodified upstream manifest is `upstream/Package.swift`.

The sole manifest change replaces Firebase's obsolete BoringSSL version range
with the sibling `../boringssl-swiftpm` source package. SwiftPM ignores an SDK's
local override when an application depends on the SDK transitively; a fresh
three-level consumer previously selected Firebase 0.7.2. Binding the edge here
makes the current provider apply at every consumer root without application
configuration. The two timestamp corrections below are the only implementation
patches. The two upstream
public-header directory symlinks are materialized as identical, individually bound
header files so package custody can reject all filesystem aliases. Their original
link targets and header origins are recorded in `UPSTREAM.json`.

All upstream targets, products, compile options and exclusions are retained,
including the two Swift build-test targets. Unselected platform implementations,
upstream non-Swift test programs and build systems are omitted; existing exclusion
entries referring to those absent files can produce SwiftPM warnings. They do not
change the enumerated target source lists. Abseil remains the upstream `jan-lts`
dependency, resolved to its supported January 2026 LTS source by the SDK lock.

Run `python3 vendor/nearby/verify_source.py` to verify the exact source closure.
`UPSTREAM.json` and the modified manifest have fixed digests in that verifier;
upgrades require reviewing both. The ordinary Swift SDK and external consumer
checks compile this package through the public mobile-transport product.

The 2026-10-28 refresh selects the current upstream commit, including the
split of incoming/outgoing payload ownership, cancellation, shutdown and
progress callbacks. Seven retained source files changed; the SwiftPM target
census, dependency revisions and package adaptation are unchanged. Upstream
publishes no supported stable release line: its only release tag remains
`v0.0.1-ios` from 2021, and it does not describe the project as an officially
supported Google product.

The refresh's 849 root-owned upstream identities match the selected upstream
tree, and all four submodule revisions remain unchanged. The source verifier
checks all 1,416 retained files, including the two explicit local patches. The rebuilt SwiftPM
product passed the seven payload-lifecycle cases in
`scripts/test_nearby_payload_lifecycle.sh`: preaccept refusal, incoming bytes,
partial-file cancellation, duplicate-ID refusal and cleanup, disconnect progress
and cleanup, shutdown cleanup, and outgoing frame reconstruction. Those checks
use the real managers, callbacks and file implementation with an in-memory
endpoint channel; they do not qualify device radios. The runner builds with the
exact `IrohaSwift/Package.resolved` graph, disables automatic resolution, and
checks the Abseil checkout revision and clean source before executing tests.

The native Apple file provider's timestamp accessors were upstream TODO stubs:
reading returned the current clock, and writing did nothing. In addition, an
incoming `Payload` cached that local time before the final chunk applied the
sender's timestamp. Two local patches correct both causes:

- `internal/platform/implementation/shared/file.cc` reads native modification
  time with `stat` and writes it with `utimensat`, preserving access time and
  integer nanosecond precision. This is the POSIX implementation selected by the
  Apple Swift package; metadata failures are logged and a failed read returns
  the Unix epoch instead of inventing a current timestamp.
- `connections/implementation/internal_payload_factory.cc` applies received
  metadata before constructing the cached `Payload`. The existing final-chunk
  application still restores modification time after the content is written.

`scripts/test_nearby_payload_lifecycle.sh` also runs four deterministic timestamp
roundtrips (before the Unix epoch, the epoch, a historical millisecond value and
2100). Exact equality is checked before the first content chunk, after intervening
writes and completion, and through a subsequent outgoing payload. Independent native file
metadata checks ensure the setter and getter cannot hide each other's defects.
The regression fails on the original provider and passes on the rebuilt package.
All 26 unchanged upstream `InternalPayloadFactoryTest` cases now pass, each in
an isolated temporary directory to avoid Apple's collision-renaming behavior.
The seven manager lifecycle cases continue to pass. No timing tolerance was
widened. These host tests do not qualify physical device radios.

Retire these local patches when upstream supplies equivalent native timestamp
handling and the exact timestamp regressions still pass. An upgrade must review
both the original and patched digests; neither source drift nor deletion of a
patch is silently accepted.
