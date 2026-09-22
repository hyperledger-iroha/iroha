# Asynchronous DA reads and authenticated proofs

The Rust SDK's four DA proof operations previously called synchronous HTTP
without the canonical account signatures required by their Torii routes. They
now belong only to `AccountClient::da()`: `prove_commitment`, `verify_commitment`,
`prove_pin_intent` and `verify_pin_intent`. Public policy, commitment, pin-intent
and manifest reads belong to `Client::da()`. All network methods are asynchronous;
the CLI uses the explicit blocking facade and its reusable runtime.

Each proof request validates direct signing authority, rejects injected witness
headers, bounds JSON encoding at 65,536 bytes before allocating the destination,
and signs the exact network, method, URL and encoded body. Address formatting is
scoped only around synchronous encoding/signing and decoding, never across an
await. Public reads strip stale canonical account headers. Response bounds,
deadlines, cancellation, media checks and structured errors share one exchange
implementation. Requests dispatch once without automatic replay.

Twelve HTTP DTOs now have one `iroha_torii_shared::da` owner. Their original
literal schema identities and binary field ordering are preserved. Shared
validation owns the canonical cursor snapshot, page maximum and exact lookup
requirements; Torii and SDK both use it. The alias byte bound belongs to the
data model. Replies bind every supplied selector and both pin authorization
network identities. Page validation checks scan bounds, strict ordering and
continuation snapshots while retaining legitimate filtered empty pages. Pin
cursors must name a positive block height within their snapshot, on both input
and output; an empty page cannot conceal a future-height continuation.
Proof location checks do not establish finality, and verification booleans are
explicitly remote results. Policy hash/version validation still needs extraction
from Core into its model owner.

The duplicate current-policy snapshot endpoint is removed from Rust, Torii,
MCP, Kotlin, Swift, the mirrored Java implementation, OpenAPI and the route
inventory. It returned the same active bundle as policy discovery; its former
"signed snapshot" description had no implementation support. Real block-authenticated
policy sidecars remain. The CLI retains only `proof-policies`. OpenAPI now
describes the existing 1,000-row rejection bound and shared selector rules.

## Qualification

The final three-library build passes with unchanged captured selected sources.
Its exact emitted harnesses pass 922 SDK, 321 shared DTO and 53 storage tests,
with ordinary stacks and no failures or ignored tests. These are 1,296 distinct
library passes, not full workspace qualification. Tests cover exact signatures
(including altered-body/network negatives), authority isolation, current-thread
executor progress, pre-dispatch validation/encoding limits, cancellation without
replay, null absence, structured HTTP/decoding failures, reply binding, filtered
pagination, blocking runtime reuse/rejection and canonical manifest artifacts.

The eight Kotlin DA tests and one Java-source consumer test pass with JDK 21
and the retained JDK 8 compile-time API restriction. The Java test checks the
retained policy GET operation, asynchronous injected transport, prefixed endpoint,
request headers and bounds, and exact typed unsigned values. Its first run caught
a raw-hex fixture hash; the fixture now uses the required checksummed hash and
the failed run is retained. This checks the retained policy operation and current Kotlin
client fixtures; it does not qualify Rust-equivalent authenticated proof behavior
or the remaining JVM pin-authorization model migration. Swift's focused DA test
launch fails at package evaluation because the required `NoritoBridge.xcframework`
is absent. Swift/native/device qualification is unverified, not passing.

The 665-operation catalog inventory, all 20 feature-resolved dependency
boundaries, codec guards and workspace formatting pass. Source-file budgets
still fail with 276 findings; no exception or limit was expanded. New production
and test modules remain below the 5,000/3,000-line limits.
All 54 operation-inventory and dependency-budget Python regressions pass.

The CLI test target compiles with unchanged selected sources in 375.95 seconds.
Its exact emitted harness passes 39 DA command tests and five manifest adapter
tests. These cover canonical public/account dispatch through asynchronous-only
transport, null proof absence, rejected requests before dispatch and rejection
of the removed snapshot command. No failures or ignored tests occurred.

The initial library build caught a missing request-builder trait import; the
second caught a test fixture field typo. Both failed captures are retained.
Strict SDK/storage test-target lint still fails: the latest capture
stops at seven existing library/feature-name diagnostics; the preceding
test-target capture also recorded 31 diagnostics outside DA. The DA changes have no reported diagnostics after correcting
the new test styles and removing an orphaned synchronous-test helper. No lint
allowance was added, and no strict-lint passing result is claimed.

The first coordinated Torii runtime selection passed 46 tests and failed 25.
Twenty-three failures exposed old fixtures mutating the lane catalog after State
had fixed its configured baseline. Those fixtures now construct their intended
Nexus catalog before opening Kura and State, preserving the production invariant.
Two expectations still named retired MCP/OpenAPI operations; they are corrected,
including the sealed hexadecimal OpenAPI test asset. A later review found the
pin-cursor height gap described above, now covered by shared, SDK and Torii
regressions. Existing list fixtures now use a committed tip containing their
seeded records; unknown-location, stale-snapshot and bounded filtered-page
assertions remain intact. Failed attempts remain recorded.

Both DA compile-fail doctests pass, proving that public and operator contexts
cannot call account proof operations. The CLI and Torii test targets compile
with unchanged captured sources (773.26 seconds including the shared build lock).

The next Torii run passed 67 of 72 tests, including all four production-route
authentication tests, the pin-cursor negative, MCP/OpenAPI contracts and all
13 manifest controls. Four visibility fixtures still changed only a local Nexus
overlay rather than authoritative canonical runtime; a fifth assertion expected
retired bundle-hash wording instead of the current tree-descriptor error.
TODO: Append the final Torii runtime result after those fixture corrections. Complete Core and release qualification belong to
their respective owners and are not inferred from these DA tests.

Captures live under
`target/architecture-redesign/da-manifest-async-2026-09-22/`. The final library
build receipt is `da-queries-build-6.json`, SHA-256
`de53ff9044a5dd4c89c563953aa094c12e4cc91c690421b3eeac9388965fc23e`.
Its runtime receipt is `da-queries-runtime-4.json`, SHA-256
`ad307eb45e8b4e99356e7386d0038fab0efe140b792c543ed6c3d28d2029650f`.
Earlier manifests retain their own [scoped correction record](async-da-manifest.md).
Signed ingest, storage-owned ingest persistence, remaining synchronous SDK
capabilities and the complete first-release redesign remain open.
