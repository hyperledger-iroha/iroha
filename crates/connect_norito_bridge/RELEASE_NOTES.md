# NoritoBridge XCFramework Artifacts

Static bridge archives explicitly bundle PQClean's common SHA3/SHAKE and
architecture-specific Keccak helpers. The Apple builder links each thin slice
into a real C consumer with every archive member loaded before staging or
publication, including slices restored from CI. Its host macOS consumer also
checks SHA3-256/SHAKE256 known answers, ML-DSA signing/verification and tamper
rejection, and ML-KEM encapsulation/decapsulation.

Current source ABI: 27. Wallet runtime installation now carries the exact
registration-source original in its C structure and JNI constructor. Artifacts
built for ABI 26 or earlier are rejected before that changed layout is called.
Wallet operations use typed native preparation through `execute` and durable
`request_status`; no generic wallet commit export is supported. The bridge exports native transaction and instruction
encoding, account admission, SoraFS reference validation, privacy proofs and
Parliament timed-OVN verification. Its Kotlin/JVM and Java/Android
`NativeSignerBridge` surface requires native-signer JNI contract revision 7.
`RegisterZkAsset` carries exactly `asset` and optional `vk_unshield`.
Transaction signing requires the genesis-derived `NetworkId`: JNI accepts
exactly 32 marked hash bytes, while the C and Swift surface accepts canonical
checksummed `NetworkId` text.

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

ABI 23 removes the archive-only Parliament timed-OVN wallet entry points. The
replacement C/JNI functions accept only a terminal canonical casting-proof
response plus caller-supplied NetworkId, finalized height/context, and ballot
attempt trust anchors. They verify finality, the fixed witness, membership,
archive replay, and exact compact binding before borrowing a wallet seed.

The archive checksums below are historical and do not establish a current
ABI-27/revision-7 artifact. Regenerate, verify, and republish the bridge
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
