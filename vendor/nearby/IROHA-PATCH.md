# Nearby Swift package ownership

This package retains Google's Nearby revision
`c80c38efe4b8a5fa72586b58d381bb546702f062` and its exact pinned JSON,
Protocol Buffers, SMHasher and UKEY2 submodule revisions in `UPSTREAM.json`.
All 356 compile and test sources from the upstream SwiftPM target description,
plus their declared header trees, transitively included headers, protobuf inputs
and licenses, are included.
Every retained upstream file matches both its Git blob identity and SHA-256.
The unmodified upstream manifest is `upstream/Package.swift`.

The sole manifest change replaces Firebase's obsolete BoringSSL version range
with the sibling `../boringssl-swiftpm` source package. SwiftPM ignores an SDK's
local override when an application depends on the SDK transitively; a fresh
three-level consumer previously selected Firebase 0.7.2. Binding the edge here
makes the current provider apply at every consumer root without application
configuration. The C, C++, Objective-C and Swift implementation is unchanged. The two upstream
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
