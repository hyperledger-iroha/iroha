# NoritoBridge XCFramework Artifacts

Current source ABI: 21. ABI 14 added
`connect_norito_encode_transfer_instruction_box` for native multisig proposal
instruction boxes; later additive revisions include the native Kagemusha V2
surfaces and the bounded SoraFS Governance DAG block/head-chain reference
validators consumed by the C# SDK. The ABI-21 Kotlin/JVM and Java/Android
`NativeSignerBridge` surface additionally requires native-signer JNI contract
revision 2. Revision 2 removes the caller-supplied `outputs` parameter from the
Kotlin/JVM and Java/Android Unshield signing descriptors; proof-authenticated
change is derived by the node. This descriptor revision is checked separately
so that an older ABI-21 artifact fails closed instead of dispatching through a
stale JNI calling convention.

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
ABI-21/revision-2 artifact. Regenerate, verify, and republish the bridge
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
