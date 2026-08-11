package org.hyperledger.iroha.android.privacy;

/** Owner-tag derivation owned by the canonical native Rust V3 implementation. */
public final class ConfidentialOwnerTag {
  private ConfidentialOwnerTag() {}

  public static byte[] defaultDiversifier() {
    return PrivacyNativeBridge.defaultConfidentialDiversifierV3();
  }

  public static byte[] deriveDiversifier(final byte[] seed) {
    return PrivacyNativeBridge.deriveConfidentialDiversifierV3(seed);
  }

  public static byte[] deriveFromSpendKey(final byte[] spendKey) {
    return deriveFromSpendKeyWithDiversifier(spendKey, defaultDiversifier());
  }

  public static byte[] deriveFromSpendKeyWithDiversifier(
      final byte[] spendKey, final byte[] diversifier) {
    return PrivacyNativeBridge.deriveConfidentialOwnerTagV3(spendKey, diversifier);
  }
}
