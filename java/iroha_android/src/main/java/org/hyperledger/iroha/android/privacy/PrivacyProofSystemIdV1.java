// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

/** Closed first-release proof-system identity in canonical Norito discriminant order. */
public enum PrivacyProofSystemIdV1 {
  STARK_FRI_SHA256_GOLDILOCKS("stark-fri-sha256-goldilocks"),
  ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255_SHA3_512(
      "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512"),
  ANONYMOUS_PGC_P256("anonymous-pgc-p256"),
  IROHA_VERANGE_P256("iroha-verange-p256"),
  VEGA_NEUTRON_NOVA_SPARTAN_HYRAX_T256("vega-neutron-nova-spartan-hyrax-t256"),
  JINDO_POLYNOMIAL_COMMITMENT("jindo-polynomial-commitment"),
  HALO2_IPA_PASTA("halo2-ipa-pasta"),
  FCMP_PLUS_PLUS_CURVE_TREE_BULLETPROOFS("fcmp-plus-plus-curve-tree-bulletproofs"),
  LANTERN_LNP22_MODULE_LINEAR_NORM("lantern-lnp22-module-linear-norm");

  private final String canonicalLabel;

  PrivacyProofSystemIdV1(final String canonicalLabel) {
    this.canonicalLabel = canonicalLabel;
  }

  public String canonicalLabel() {
    return canonicalLabel;
  }

  public static PrivacyProofSystemIdV1 fromCanonicalLabel(final String label) {
    if (label != null) {
      for (final PrivacyProofSystemIdV1 value : values()) {
        if (value.canonicalLabel.equals(label)) {
          return value;
        }
      }
    }
    throw new IllegalArgumentException("unknown canonical privacy proof-system id");
  }
}
