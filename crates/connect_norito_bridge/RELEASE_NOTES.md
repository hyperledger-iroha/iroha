# NoritoBridge XCFramework Artifacts

Current source ABI: 23. ABI 14 added
`connect_norito_encode_transfer_instruction_box` for native multisig proposal
instruction boxes; later additive revisions include the bounded Kagemusha V1
wire validators and SoraFS Governance DAG block/head-chain reference
validators consumed by the C# SDK. The ABI-23 Kotlin/JVM and Java/Android
`NativeSignerBridge` surface additionally requires native-signer JNI contract
revision 5. Revision 4 sealed the removal of generic `Shield`, `ZkTransfer`, and
`Unshield` transaction encoders plus native anonymous-escrow and authority-free
Kaigi helpers from the C and JNI surfaces. The bridge retains specialized
Kagemusha V1 raw/text validators for request, acceptance authorization,
ticket, payment, acknowledgement, mint authorization, bound mint credit, and
redemption voucher. The source surface also validates the authenticated
no-commit recovery closure and the complete five-message exchange as exact
cross-bound objects, plus fail-closed device-lifecycle probes. `RegisterZkAsset`
now carries exactly
`asset`, `vk_unshield`, and `vk_shield`; optional key presence enables each
settlement role, with no mode, boolean enablement, or asset-bound transfer-key
field. The JNI contract revision is checked separately so an artifact that is
not exact ABI 23 fails closed instead of exposing a retired transaction surface.
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

ABI 23 removes the archive-only Parliament timed-OVN wallet entry points. The
replacement C/JNI functions accept only a terminal canonical casting-proof
response plus caller-supplied NetworkId, finalized height/context, and ballot
attempt trust anchors. They verify finality, the fixed witness, membership,
archive replay, and exact compact binding before borrowing a wallet seed.

The archive checksums below are historical and do not establish a current
ABI-23/revision-5 artifact. Regenerate, verify, and republish the bridge
artifacts before cutting an SDK release that depends on the current source
surface.

- `NoritoBridge.xcframework.zip`
  - SHA-256: 9bdd96f97f2eccc9e901c0500bd8f2b046c600080ebbe1213a4febba13c44efd
- `NoritoBridge-xcframework.tar.gz`
  - SHA-256: 316fe22a83f217180700e1aa8b98c00d001d8009dfbdf465717794878c75441c

Instructions:
1. Package the authenticated archive with
   `scripts/package_mobile_sdk_artifacts.sh --apple`; do not reuse the historical
   hashes above.
2. Publish the generated canonical
   `NoritoBridge-v<version>.xcframework.zip`. The package owner invokes
   `scripts/render_norito_bridge_podspec.py` to compute its exact SHA-256 and
   create `NoritoBridge-<version>.podspec`; do not hand-edit template tokens.
3. Publish the generated binary spec before the same-version `IrohaSwift` source
   spec and retain the signed artifact/provenance inventory.
