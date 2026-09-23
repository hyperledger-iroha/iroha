# MKHE native40 source and qPCS gap audit — 2026-09-23

This is a read-only source review of the `optimizations` working tree. It is not
an immutable build receipt or a release qualification record. No Rust source was
changed and no Cargo command was run for this audit.

## First missing production producer

The governed [release profile](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/manifest.rs)
still selects 38 moduli in `RELEASE_MODULI_V1` and `release_profile_v1`.
The [streaming collective owner](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source.rs)
derives its limb count from that release array: it publishes 38 `A` and 38 `B`
limb objects, then 38 `C0` and 38 `C1` objects for each of 43 Phase-23 records.
That is `38 × (2 + 2 × 43) = 3,344` real objects. The native40
[publisher](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_public_polynomial_publisher.rs)
requires `40 × (2 + 2 × 43) = 3,520` distinct authenticated objects: **176
objects are absent**. Its exact move-only
`RnsNativePublicPolynomialCoefficientSourceV1` contract exists, but the sole
production adapter, `RnsNativePhase23FortyLimbProductionSourceV1`, is an
uninhabited enum. `publish_rns_native_public_polynomials_v1` therefore has no
production source to consume. The upstream
[Phase-23 materialize/encrypt owner](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23.rs)
retains 43 manifests and the original confidential source, but its
`Phase23ContextCorrespondenceSealV1` is also uninhabited in production.

The publisher's typed `into_reader_handoff_v1` connects to the authenticated
[reader](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_public_polynomial_reader.rs),
but the reader's upstream-manifest, source-preflight and direct-numeric-source
integration flags remain false. In particular,
`preflight_rns_native_rlwe_source_statement_v1` still accepts a detached
`RnsNativePublicArtifactViewV1` of digest facts alongside its confidential
snapshot in the [source-statement module](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_rlwe_source_statement.rs).
Its authenticated replay checks source encoding, nonce binding and snapshot
identity; it does not establish that the detached public facts came from the
same live 3,520-object publication. The
[private source-chain composite seam](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_composite_verifier.rs)
is non-authorizing; the public `verify_zk_ams_mkhe_rns_native_composite_v1`
still returns `StageUnavailable` on production stage paths.

## Fixed qPCS obstruction

The [qPCS tree work test](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_tree.rs)
derives **505,349,817,048 field operations** even for the one-payload
diagnostic lower bound of the initial tree. That already exceeds the unchanged
**128,000,000,000-unit** work ceiling in the
[native profile](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_profile.rs).
The all-payload initial-tree count is 3,032,072,370,498 field operations. A
publisher/reader handoff cannot make this qPCS construction admissible; the
commitment/evaluation design needs separate review within the existing whole
proof and resource limits.

## Narrow next slice and evidence limits

The first reviewable native40 slice is a single governed, ordered 40-prime/root
profile owner used by release and native consumers, followed by source-owned
publication of real `A`, `B`, `C0` and `C1` coefficient chunks from the same
key and all 43 encryptions. Keep the production adapter and readiness closed
until that owner can move its authenticated source and CAS provider through the
publisher into the reader. Reconcile the full `BgvProfile` identity and retire
the 38-limb selection and basis-extension path; do not infer two limbs or add a
compatibility shim. The existing
[publisher tests](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_public_polynomial_publisher_tests.rs)
pin the missing 176 objects and uninhabited adapter. The
[reader tests](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_public_polynomial_reader_tests.rs)
check manifest order, canonical residue evaluation and fail-closed reads with
fixture providers. Neither suite supplies governed full40 KATs, whole-session
I/O/work/RSS measurements, independent security/noise evidence or a complete
source-bound proof.

No 38-to-40 migration, cryptographic qualification or production composite
admission was completed by this audit.
