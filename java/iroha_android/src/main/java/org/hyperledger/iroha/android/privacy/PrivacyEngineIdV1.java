// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

/** Closed first-release native-engine identity in canonical Norito discriminant order. */
public enum PrivacyEngineIdV1 {
  NATIVE_GOLDILOCKS_STARK_FRI("native-goldilocks-stark-fri"),
  NATIVE_ZK_AMS_MASKED_RELAXED_SPARTAN_T256_RISTRETTO255(
      "native-zk-ams-masked-relaxed-spartan-t256-ristretto255"),
  NATIVE_ANONYMOUS_PGC_P256("native-anonymous-pgc-p256"),
  NATIVE_VERANGE_P256("native-verange-p256"),
  NATIVE_VEGA("native-vega"),
  NATIVE_JINDO("native-jindo"),
  NATIVE_HALO2_ORCHARD("native-halo2-orchard"),
  NATIVE_FCMP_PLUS_PLUS("native-fcmp-plus-plus"),
  NATIVE_LANTERN_LNP22("native-lantern-lnp22");

  private final String canonicalLabel;

  PrivacyEngineIdV1(final String canonicalLabel) {
    this.canonicalLabel = canonicalLabel;
  }

  public String canonicalLabel() {
    return canonicalLabel;
  }

  public static PrivacyEngineIdV1 fromCanonicalLabel(final String label) {
    if (label != null) {
      for (final PrivacyEngineIdV1 value : values()) {
        if (value.canonicalLabel.equals(label)) {
          return value;
        }
      }
    }
    throw new IllegalArgumentException("unknown canonical privacy engine id");
  }
}
