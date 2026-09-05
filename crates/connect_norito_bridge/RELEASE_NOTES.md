# NoritoBridge XCFramework Artifacts

Static bridge archives explicitly bundle PQClean's common SHA3/SHAKE and
architecture-specific Keccak helpers. The Apple builder links each thin slice
into a real C consumer with every archive member loaded before staging or
publication, including slices restored from CI. Its host macOS consumer also
checks SHA3-256/SHAKE256 known answers, ML-DSA signing/verification and tamper
rejection, and ML-KEM encapsulation/decapsulation.

Current source ABI: 23. ABI 14 added
`connect_norito_encode_transfer_instruction_box` for native multisig proposal
instruction boxes; later additive revisions include the bounded KAGEMUSHA V1
wire validators and SoraFS Governance DAG block/head-chain reference
validators consumed by the C# SDK. The ABI-23 Kotlin/JVM and Java/Android
`NativeSignerBridge` surface additionally requires native-signer JNI contract
revision 5. Revision 4 sealed the removal of generic `Shield`, `ZkTransfer`, and
`Unshield` transaction encoders plus native anonymous-escrow and authority-free
Kaigi helpers from the C and JNI surfaces. The bridge retains specialized
KAGEMUSHA raw/text validators for the sole ordered IPM1 lifecycle—request (`1`),
payment (`2`), and durable acknowledgement (`3`)—plus mint authorization, bound
mint credit, and redemption voucher. Every progressive validator requires the
exact preceding messages, and the exchange validator enforces the complete
three-message binding and aggregate caps. The fail-closed device surface exposes
the sole contiguous 22-operation lifecycle and exact `0x0000ffff` capability mask.
The generic C and JNI execute paths now parse the complete `IKGMJCM1` frame and
reject bad magic, version, operation, flags, request ID, length, digest, or
suffix before reporting service availability. Receiver operations use distinct
bounded canonical Norito schemas and exact credit-ID recovery selectors; their
reply validators bind the staged request/payment bytes and receipt context and
retain full-width `u128` inbox revisions. Operation 16 has shared canonical
mint-stage command/result bodies and two structural C validation exports. Both
C and JNI reject malformed operation-16 bodies before returning unavailable.
The command binds the exact pre-debit authorization and finalized mint credit;
the result binds the same credit ID with a closed new-stage/duplicate status.
Private reservation openings and full Guard certificates remain native-only.
The current exact artifact inventory requires the current KAGEMUSHA C exports,
including both mint-stage validators; previous binaries do not satisfy it.
This is wire and dispatcher code only:
the stock capabilities and valid execution results remain unavailable, and no
test engine, host flag, or shape-valid reply grants monetary authority.
`RegisterZkAsset`
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
