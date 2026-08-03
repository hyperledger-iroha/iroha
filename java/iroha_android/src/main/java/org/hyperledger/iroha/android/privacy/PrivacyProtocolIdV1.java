// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

/** Closed first-release protocol identity in canonical Norito discriminant order. */
public enum PrivacyProtocolIdV1 {
  ZK_ACE_PQ_AUTHORIZATION_V0(
      "zk-ace-pq-authorization-v0",
      PrivacyProofSystemIdV1.STARK_FRI_SHA256_GOLDILOCKS,
      PrivacyEngineIdV1.NATIVE_GOLDILOCKS_STARK_FRI),
  ANONYMOUS_PGC_K_OUT_OF_N_V1(
      "anonymous-pgc-k-out-of-n-v1",
      PrivacyProofSystemIdV1.ANONYMOUS_PGC_P256,
      PrivacyEngineIdV1.NATIVE_ANONYMOUS_PGC_P256),
  VERANGE_TRANSPARENT_RANGE_V1(
      "verange-transparent-range-v1",
      PrivacyProofSystemIdV1.IROHA_VERANGE_P256,
      PrivacyEngineIdV1.NATIVE_VERANGE_P256),
  IROHA_ZK_AMS_V1(
      "iroha-zk-ams-v1",
      PrivacyProofSystemIdV1.ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255_SHA3_512,
      PrivacyEngineIdV1.NATIVE_ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255),
  VEGA_EXISTING_CREDENTIAL_ZK_V0(
      "vega-existing-credential-zk-v0",
      PrivacyProofSystemIdV1.VEGA_NEUTRON_NOVA_SPARTAN_HYRAX_T256,
      PrivacyEngineIdV1.NATIVE_VEGA),
  IROHA_ZK_X509_STARK_P256_V0(
      "iroha-zk-x509-stark-p256-v0",
      PrivacyProofSystemIdV1.STARK_FRI_SHA256_GOLDILOCKS,
      PrivacyEngineIdV1.NATIVE_GOLDILOCKS_STARK_FRI),
  IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0(
      "iroha-jindo-polynomial-commitment-v0",
      PrivacyProofSystemIdV1.JINDO_POLYNOMIAL_COMMITMENT,
      PrivacyEngineIdV1.NATIVE_JINDO),
  IROHA_BOOTLE_LANTERN_ANONCRED_V1(
      "iroha-bootle-lantern-anoncred-v1",
      PrivacyProofSystemIdV1.LANTERN_LNP22_MODULE_LINEAR_NORM,
      PrivacyEngineIdV1.NATIVE_LANTERN_LNP22),
  ORCHARD_HALO2_ACTIONS_V1(
      "orchard-halo2-actions-v1",
      PrivacyProofSystemIdV1.HALO2_IPA_PASTA,
      PrivacyEngineIdV1.NATIVE_HALO2_ORCHARD),
  MONERO_FCMP_PLUS_PLUS_V1(
      "monero-fcmp-plus-plus-v1",
      PrivacyProofSystemIdV1.FCMP_PLUS_PLUS_CURVE_TREE_BULLETPROOFS,
      PrivacyEngineIdV1.NATIVE_FCMP_PLUS_PLUS),
  IROHA_IVM_PRIVATE_NOTE_STARK_V1(
      "iroha-ivm-private-note-stark-v1",
      PrivacyProofSystemIdV1.STARK_FRI_SHA256_GOLDILOCKS,
      PrivacyEngineIdV1.NATIVE_GOLDILOCKS_STARK_FRI),
  PQ_MASP_STARK_V0(
      "pq-masp-stark-v0",
      PrivacyProofSystemIdV1.STARK_FRI_SHA256_GOLDILOCKS,
      PrivacyEngineIdV1.NATIVE_GOLDILOCKS_STARK_FRI);

  private final String canonicalLabel;
  private final PrivacyProofSystemIdV1 expectedProofSystem;
  private final PrivacyEngineIdV1 expectedEngine;

  PrivacyProtocolIdV1(
      final String canonicalLabel,
      final PrivacyProofSystemIdV1 expectedProofSystem,
      final PrivacyEngineIdV1 expectedEngine) {
    this.canonicalLabel = canonicalLabel;
    this.expectedProofSystem = expectedProofSystem;
    this.expectedEngine = expectedEngine;
  }

  public String canonicalLabel() {
    return canonicalLabel;
  }

  public PrivacyProofSystemIdV1 expectedProofSystem() {
    return expectedProofSystem;
  }

  public PrivacyEngineIdV1 expectedEngine() {
    return expectedEngine;
  }

  public static PrivacyProtocolIdV1 fromCanonicalLabel(final String label) {
    if (label != null) {
      for (final PrivacyProtocolIdV1 value : values()) {
        if (value.canonicalLabel.equals(label)) {
          return value;
        }
      }
    }
    throw new IllegalArgumentException("unknown canonical privacy protocol id");
  }
}
