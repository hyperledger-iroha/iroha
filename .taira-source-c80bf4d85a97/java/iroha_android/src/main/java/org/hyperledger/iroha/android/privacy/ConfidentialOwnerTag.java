package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;
import java.util.Collections;

/** Owner-tag derivation matching {@code derive_confidential_owner_tag_v2} in Rust. */
public final class ConfidentialOwnerTag {
  private ConfidentialOwnerTag() {}

  public static byte[] defaultDiversifier() {
    return ConfidentialNoteScalars.scalarToLittleEndian(BigInteger.ONE);
  }

  public static byte[] deriveDiversifier(final byte[] seed) {
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.diversifier", Collections.singletonList(seed.clone())));
  }

  public static byte[] deriveFromSpendKey(final byte[] spendKey) {
    return deriveFromSpendKeyWithDiversifier(spendKey, defaultDiversifier());
  }

  public static byte[] deriveFromSpendKeyWithDiversifier(
      final byte[] spendKey, final byte[] diversifier) {
    final BigInteger spendScalar =
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.spend_scalar",
            Collections.singletonList(ConfidentialNoteScalars.copyNonEmpty(spendKey, "spendKey")));
    final BigInteger diversifierScalar =
        ConfidentialNoteScalars.littleEndianScalar(diversifier, "diversifier");
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.poseidonPair(spendScalar, diversifierScalar));
  }
}
