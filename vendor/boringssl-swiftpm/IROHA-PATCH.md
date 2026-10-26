# BoringSSL source package for Nearby

This Swift package supplies Nearby's `openssl_grpc` product from unmodified
[BoringSSL revision `90c6b370770adce64ff435789bfd1940067df415`](https://boringssl.googlesource.com/boringssl/+/90c6b370770adce64ff435789bfd1940067df415),
dated 2026-10-22. `UPSTREAM.json` records the Git tree and SHA-256 of every copied
upstream file. BoringSSL's Apache license and Fiat's license are retained.

Run `python3 vendor/boringssl-swiftpm/verify_source.py` from the repository root
to validate every hash, the exact generated source closure, normalized paths,
the reviewed package manifest, and the absence of missing or additional inputs.
The negative controls are in
`pytests/scripts/boringssl_swiftpm_source_test.py`. A refresh must explicitly
update the verifier's reviewed metadata and manifest hashes and source census.

The Firebase SwiftPM 0.41.0 wrapper still embeds April 2025 BoringSSL, and its
current branch does not update the runtime sources. Nearby also retains an old
Firebase version requirement. The repository therefore owns this small packaging
layer and its source refreshes; this is not a new Firebase release or an upstream
support promise.

`Package.swift` reads the upstream-generated `gen/sources.json` and selects the
`bcm`, `crypto`, and `ssl` source and assembly groups. The copied closure contains
their `srcs`, `internal_hdrs`, `hdrs`, and `asm` entries. Assembly keeps upstream
architecture/platform guards and runtime CPU dispatch; portable implementations
remain present. No cryptographic implementation or generated assembly is patched.
The C++ options match upstream's aliasing and runtime requirements. The Apple
privacy resource is unchanged from the identified Firebase package and has a
separate provenance entry.

To refresh, obtain an immutable upstream commit, copy exactly those four fields
from the three source groups plus `gen/sources.json` and both license files,
and regenerate the file hashes and Git tree in `UPSTREAM.json`. Run the upstream
`go run ./util/pregenerate err_data.cc`; its output must match the committed
`gen/crypto/err_data.cc`. Review the upstream source-list/build-option changes
before changing the package selection. Do not copy `.git`, build outputs,
test-only dependencies, or an old generated error table into the package.

Qualification requires the following on the selected upstream checkout:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build build --target crypto_test ssl_test -j2
./build/crypto_test
./build/ssl_test
cmake -S . -B build-portable -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=ON -DOPENSSL_NO_ASM=1
cmake --build build-portable --target crypto_test -j2
./build-portable/crypto_test
```

Build this Swift package, resolve the real Nearby product with this local package
identity, and test P256 UKEY2 handshakes, fresh session keys, authenticated binary
roundtrips, tampering, replay, and malformed messages using the linked objects.
Finally run the repository's canonical `python3.12 scripts/test_swift_local.py`.
These component checks do not replace physical-device qualification.
