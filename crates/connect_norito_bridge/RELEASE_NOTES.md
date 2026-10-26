# NoritoBridge XCFramework Artifacts

Static bridge archives retain the pure-Rust ML-DSA/ML-KEM providers. The Apple
builder rejects retired PQClean objects and duplicate archive members, binds the
locked RustCrypto dependencies to the successful Cargo build, and links every
archive member into a real C consumer before staging or publication. Its host
consumer executes all three ML-DSA and ML-KEM parameter sets through the public
ABI, including signature and ciphertext tamper checks. Known-answer vectors remain
in the source-owned Rust suite.

Current source ABI: 33. Every SDK and artifact manifest must admit exactly ABI 33
and the complete required export surface. Other bridge versions are rejected before
calling the native layout. Matching rebuilt artifacts and consumer execution remain
required; source checks alone do not qualify a release.

KAGEMUSHA mutation and review calls carry the exact exchange identity. Synchronous
begin reserves exchange occupancy before queueing; end fences new calls while
accepted calls retain their owner through response delivery. The native maintenance
worker owns deferred receipt verification, folding and guarded Archive admission.
Direct receipt-to-Archive selectors and the public raw commit path are removed.

Send and Unload review requests bind the request ID and either a fresh expectation or
the exact original, unstarted projection. Issuance returns an actual review capability
(result 18), an authenticated existing request (66), or a tokenless maintenance wait
(67). Execute consumes the actual capability with its privately bound request ID;
only typed result 65 retains that same live capability for a retry. Projection bytes
are DATA and cannot reconstruct a capability. The C review structure and JNI review
arguments require the request ID and expectation fields; execute accepts no separate
request ID. SDKs use the complete KWORV1 projection, including the canonical destination
account original for Send, with no retired-layout decoder.

The bridge exports 29 KAGEMUSHA wallet C entry points, including exchange ownership,
review, observation, account admission and enrollment refusal. Enrollment selector 3
accepts the Android KeyMint chain only. The first-device authentication-key JNI owner,
Play Integrity token input and Parliament timed-OVN wallet exports are removed.
Abandon returns result 58 for a local no-marker abandonment or result 28 with the exact
signed Abandonment frame. Issuer refusals use the closed native refusal table.
Capacity failures are typed: -12 is NotEnoughStorage and -14 is NotEnoughMemory;
installed-relation bounds are checked before proving and before commit.

Native transaction and instruction encoding, account admission, SoraFS reference
validation and privacy proofs share the exact ABI 33 bridge. The Kotlin-owned
`NativeSignerBridge` requires native-signer JNI contract revision 7.
`RegisterZkAsset` carries exactly `asset` and optional `vk_unshield`. Transaction
signing requires the genesis-derived `NetworkId`: JNI accepts exactly 32 marked
hash bytes, while C and Swift accept canonical checksummed `NetworkId` text.

Canonical KAGEMUSHA wallet objects are owned by
`iroha_data_model::kagemusha::kagemusha_wallet_v1`. The bridge exposes no
KAGEMUSHA coordinator or device runtime. Native wallet integration and device
qualification remain open.

The exact-12 privacy KAT ABI is compiled through the narrow
`iroha_data_model/privacy-exact12-conformance` feature. Shipping bridge builds
do not enable the data model's general `test-fixtures` feature, random-key
feature edge, or block-tampering helpers.

The native privacy metadata ABI exports only the local
`PrivacyCompiledProfileCatalogV1`. It intentionally has no committed height,
policy, activation, lifecycle, or readiness projection. SDKs must fetch a
fresh authoritative `PrivacyCapabilitySnapshotV1` from live Torii before
submitting a privacy proof.

The archive checksums below are historical and do not establish a current
ABI-33/revision-7 artifact. Regenerate, verify, and republish the bridge
artifacts before cutting an SDK release that depends on the current source
surface.

- `NoritoBridge.xcframework.zip`
  - SHA-256: 9bdd96f97f2eccc9e901c0500bd8f2b046c600080ebbe1213a4febba13c44efd
- `NoritoBridge-xcframework.tar.gz`
  - SHA-256: 316fe22a83f217180700e1aa8b98c00d001d8009dfbdf465717794878c75441c

Instructions:
1. Package the authenticated archive with
   `scripts/package_mobile_sdk_artifacts.sh --apple --lockfile-path <canonical-external-Cargo.lock>`;
   retain exact source, lock, tool and package provenance. Do not reuse the
   historical hashes above.
2. Publish the generated canonical
   `NoritoBridge-v<version>.xcframework.zip`, whose version comes only from
   `IrohaSwift/VERSION`, with its authenticated manifest and checksum inventory.
3. Qualify the ZIP with SwiftPM and an ordinary `IrohaSwift` dependency in
   Release, including native execution without unsafe linker flags. SwiftPM is
   the sole supported Swift packaging path. Public installation and signed
   release evidence remain distinct from local host/simulator validation; see
   [the release contract](../../docs/norito_bridge_release.md).
