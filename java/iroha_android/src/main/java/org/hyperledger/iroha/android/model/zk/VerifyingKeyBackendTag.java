package org.hyperledger.iroha.android.model.zk;

import java.util.Locale;

/**
 * Backend identifiers for verifying key records.
 *
 * <p>Matches the Norito enums exposed by {@code iroha_data_model::zk::BackendTag}.
 */
public enum VerifyingKeyBackendTag {
  HALO2_IPA_PASTA("halo2-ipa-pasta"),
  HALO2_BN254("halo2-bn254"),
  GROTH16("groth16"),
  STARK("stark"),
  UNSUPPORTED("unsupported");

  private final String noritoValue;

  VerifyingKeyBackendTag(final String noritoValue) {
    this.noritoValue = noritoValue;
  }

  /** Returns the Norito string used in encoded instruction arguments. */
  public String noritoValue() {
    return noritoValue;
  }

  /** Parses a Norito backend string into the corresponding enum value. */
  public static VerifyingKeyBackendTag parse(final String value) {
    if (value == null) {
      throw new IllegalArgumentException("backend tag must not be null");
    }
    final String normalized = value.trim().toLowerCase(Locale.ROOT);
    for (final VerifyingKeyBackendTag tag : values()) {
      if (tag.noritoValue.equals(normalized)) {
        return tag;
      }
    }
    throw new IllegalArgumentException("unsupported backend tag: " + value);
  }
}

