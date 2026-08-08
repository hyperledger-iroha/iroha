# NoritoBridge XCFramework Artifacts

Current source ABI: 22. ABI 14 added
`connect_norito_encode_transfer_instruction_box` for native multisig proposal
instruction boxes; later additive revisions include the native Kagemusha V2
surfaces and the bounded SoraFS Governance DAG block/head-chain reference
validators consumed by the C# SDK. The ABI-22 Kotlin/JVM and Java/Android
`NativeSignerBridge` surface additionally requires native-signer JNI contract
revision 5. Revision 4 sealed the removal of generic `Shield`, `ZkTransfer`, and
`Unshield` transaction encoders plus native anonymous-escrow and authority-free
Kaigi helpers from the C and JNI surfaces. The bridge retains specialized
Kagemusha proof and settlement helpers. `RegisterZkAsset` now carries exactly
`asset`, `vk_unshield`, and `vk_shield`; optional key presence enables each
settlement role, with no mode, boolean enablement, or asset-bound transfer-key
field. The JNI contract revision is checked separately so an older ABI-22
artifact fails closed instead of exposing a retired transaction surface.
Revision 5 hard-cuts native transaction signing from human chain labels to the
exact genesis-derived `NetworkId`: JNI accepts exactly 32 marked hash bytes,
while the C and Swift surface accepts only canonical checksummed `NetworkId`
text. No label conversion or compatibility fallback exists.

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
ABI-22/revision-5 artifact. Regenerate, verify, and republish the bridge
artifacts before cutting an SDK release that depends on the current source
surface.

- `NoritoBridge.xcframework.zip`
  - SHA-256: 9bdd96f97f2eccc9e901c0500bd8f2b046c600080ebbe1213a4febba13c44efd
- `NoritoBridge-xcframework.tar.gz`
  - SHA-256: 316fe22a83f217180700e1aa8b98c00d001d8009dfbdf465717794878c75441c

Instructions:
1. Upload one of the archives to the release host.
2. Update `Package.swift.template` `<URL>` + `<CHECKSUM>` and `NoritoBridge.podspec.template` `<ZIP_URL>`.
3. Commit/publish templates for consumers.
